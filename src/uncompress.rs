use aliases::{ByteStream};
use bytes::{Buf, Bytes, IntoBuf, LittleEndian};
use futures::{Async, Poll, Stream};
use snap;
use std::collections::VecDeque;
use std::convert::TryFrom;
use std::io;

use shared::{crc32c_masked, FrameType};

// special snappy stream magic header
const MAGIC: &'static [u8] = b"sNaPpY";

#[derive(PartialEq)]
enum State {
  // reading the first 4 byte header
  Header,

  // reading the body of the frame
  Body { frame_type: Result<FrameType, u8>, length: usize }
}

pub struct SnappyUncompress<S> where S: ByteStream {
  stream: S,
  decoder: snap::Decoder,
  state: State,

  // buffer incoming data until we have a full frame
  saved: VecDeque<Bytes>,
  saved_length: usize,

  // snappy framed streams require a magic header (at least once)
  seen_magic: bool,
}

impl<S> SnappyUncompress<S> where S: ByteStream {
  pub fn new(stream: S) -> SnappyUncompress<S> {
    SnappyUncompress {
      stream,
      decoder: snap::Decoder::new(),
      state: State::Header,
      saved: VecDeque::new(),
      saved_length: 0,
      seen_magic: false,
    }
  }

  fn feed(&mut self) -> Option<Poll<Option<Bytes>, io::Error>> {
    match self.stream.poll() {
      Ok(Async::Ready(None)) => {
        if self.state == State::Header && self.saved_length == 0 {
          Some(Ok(Async::Ready(None)))
        } else {
          Some(Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Truncated snappy frame")))
        }
      },
      Ok(Async::Ready(Some(data))) => {
        self.saved_length += data.len();
        self.saved.push_back(data);
        None
      }
      other => Some(other)
    }
  }

  // pop saved buffers until we have the requested amount, then pack them
  // into a single Bytes (probably with copying, boo).
  fn drain(&mut self, count: usize) -> Bytes {
    let mut drained: Vec<Bytes> = Vec::new();
    let mut drained_length = 0;

    while drained_length < count {
      let b = self.saved.pop_front().unwrap();
      if drained_length + b.len() <= count {
        drained_length += b.len();
        self.saved_length -= b.len();
        drained.push(b);
      } else {
        // split last Bytes object to get an exact count.
        let n = count - drained_length;
        drained_length += n;
        self.saved_length -= n;
        drained.push(b.slice(0, n));
        self.saved.push_front(b.slice_from(n));
      }
    }

    if drained.len() == 0 {
      Bytes::new()
    } else if drained.len() == 1 {
      drained[0].clone()
    } else {
      // unavoidable copy here. we could build a rope out of the segments,
      // but snappy will want to take slices. just suck it up and copy.
      let mut rv: Vec<u8> = Vec::with_capacity(drained_length);
      for ref b in &drained { rv.extend(b.as_ref()) };
      Bytes::from(rv)
    }
  }

  fn process_frame(&mut self, frame_type: Result<FrameType, u8>, data: Bytes) -> Result<Option<Bytes>, io::Error> {
    println!("Let us decode {:?} len {}", frame_type, data.len());

    // some error cases first: expect to have seen at least one magic header, and a known frame type.
    if !self.seen_magic && frame_type != Ok(FrameType::Stream) {
      return Err(io::Error::new(io::ErrorKind::InvalidData, "Not a snappy stream (missing magic header)"));
    }

    match frame_type {
      Err(b) if 0x02 <= b && b <= 0x7f => {
        Err(io::Error::new(io::ErrorKind::InvalidData, format!("Unknown chunk type {}", b)))
      },

      Ok(FrameType::Stream) => {
        if data.as_ref() != MAGIC {
          Err(io::Error::new(io::ErrorKind::InvalidData, "Not a snappy stream (mangled magic header)"))
        } else {
          // skip.
          self.seen_magic = true;
          Ok(None)
        }
      },

      Ok(FrameType::Uncompressed) => {
        let out = data.slice_from(4);
        let expected_crc = data.into_buf().get_u32::<LittleEndian>();
        let crc = crc32c_masked(out.as_ref());
        if crc != expected_crc {
          let message = format!("Frame CRC mismatch: expected {:x}, got {:x}", expected_crc, crc);
          Err(io::Error::new(io::ErrorKind::InvalidData, message))
        } else {
          Ok(Some(out))
        }
      },

      Ok(FrameType::Compressed) => {
        let compressed = data.slice_from(4);
        let expected_crc = data.into_buf().get_u32::<LittleEndian>();
        match self.decoder.decompress_vec(compressed.as_ref()) {
          Err(e) => Err(io::Error::new(io::ErrorKind::InvalidData, e)),
          Ok(uncompressed) => {
            let crc = crc32c_masked(uncompressed.as_ref());
            if crc != expected_crc {
              let message = format!("Frame CRC mismatch: expected {:x}, got {:x}", expected_crc, crc);
              Err(io::Error::new(io::ErrorKind::InvalidData, message))
            } else {
              Ok(Some(Bytes::from(uncompressed)))
            }
          }
        }
      },

      // anything else can be skipped:
      _ => Ok(None)
    }
  }
}

impl<S> Stream for SnappyUncompress<S> where S: ByteStream {
  type Item = Bytes;
  type Error = io::Error;

  fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
    loop {
      match self.state {
        State::Header => {
          if self.saved_length < 4 {
            match self.feed() {
              Some(rv) => return rv,
              None => ()
            }
          } else {
            let mut header = self.drain(4).into_buf();
            let frame_type = FrameType::try_from(header.get_u8());
            let length = header.get_uint::<LittleEndian>(3) as usize;
            self.state = State::Body { frame_type, length };
            // loop around and try again.
          }
        },
        State::Body { frame_type, length } => {
          if self.saved_length < length {
            match self.feed() {
              Some(rv) => return rv,
              None => ()
            }
          } else {
            let data = self.drain(length);
            match self.process_frame(frame_type, data) {
              Err(e) => return Err(e),
              Ok(None) => {
                // skippable frame, loop around.
                self.state = State::Header;
              },
              Ok(Some(data)) => {
                self.state = State::Header;
                return Ok(Async::Ready(Some(data)));
              }
            }
          }
        }
      }
    }
  }
}
