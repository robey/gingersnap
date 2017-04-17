use aliases::{ByteStream};
use bytes::{BufMut, Bytes, BytesMut, LittleEndian};
use crc::crc32;
use futures::{Async, Poll, Stream};
use snap;
use std::io;

// private inside snap :(
const MAX_BLOCK_SIZE: usize = 1 << 16;

lazy_static! {
  static ref MAX_COMPRESS_BLOCK_SIZE: usize = snap::max_compress_len(MAX_BLOCK_SIZE);
}

// An enumeration describing each of the 4 main chunk types.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ChunkType {
  Stream = 0xff,
  Compressed = 0x00,
  Uncompressed = 0x01,
  Padding = 0xfe,
}

// special snappy stream magic header
const MAGIC: &'static [u8] = b"\xFF\x06\x00\x00sNaPpY";

pub struct SnappyCompress<S> where S: ByteStream {
  stream: S,
  encoder: snap::Encoder,

  // we can only compress MAX_BLOCK_SIZE at a time, so if we receive a
  // larger block, we'll need to generate more than one outbound buffer.
  // this stores the remainder for next time.
  current_buffer: Option<Bytes>,

  output_buffer: Vec<u8>,

  // snappy framed streams require a magic header (at least once)
  sent_magic: bool,
}

impl<S> SnappyCompress<S> where S: ByteStream {
  pub fn new(stream: S) -> SnappyCompress<S> {
    let mut s = SnappyCompress {
      stream,
      encoder: snap::Encoder::new(),
      current_buffer: None,
      output_buffer: Vec::with_capacity(*MAX_COMPRESS_BLOCK_SIZE),
      sent_magic: false,
    };
    // fill the output buffer with zeros for safety.
    s.output_buffer.resize(*MAX_COMPRESS_BLOCK_SIZE, 0);
    s
  }

  fn encode_frame(&mut self, data: Bytes) -> Result<Bytes, snap::Error> {
    let crc = crc32c_masked(data.as_ref());
    // this can't really fail, but roll with it:
    let length = self.encoder.compress(data.as_ref(), &mut self.output_buffer[..])?;

    // if the result is >= 7/8 of the original size, skip compression.
    if length >= data.len() - (data.len() / 8) {
      let mut out = BytesMut::with_capacity(data.len() + 8);
      Self::encode_header(&mut out, ChunkType::Uncompressed, data.len() + 4, crc);
      out.put(data);
      Ok(out.freeze())
    } else {
      let mut out = BytesMut::with_capacity(length + 8);
      Self::encode_header(&mut out, ChunkType::Compressed, length + 4, crc);
      out.put(&self.output_buffer[..length]);
      Ok(out.freeze())
    }
  }

  // header: type(1), len_le(3), crc_le(4)
  fn encode_header(out: &mut BytesMut, chunk_type: ChunkType, length: usize, crc: u32) {
    out.put_uint::<LittleEndian>(chunk_type as u8 as u64, 1);
    out.put_uint::<LittleEndian>(length as u64, 3);
    out.put_u32::<LittleEndian>(crc);
  }
}

impl<S> Stream for SnappyCompress<S> where S: ByteStream {
  type Item = Bytes;
  type Error = io::Error;

  fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
    if !self.sent_magic {
      self.sent_magic = true;
      return Ok(Async::Ready(Some(Bytes::from_static(MAGIC))));
    }

    if self.current_buffer.is_none() {
      match self.stream.poll() {
        Ok(Async::Ready(Some(data))) => {
          self.current_buffer = Some(data);
        },
        other => {
          return other;
        }
      }
    }

    let mut buffer = self.current_buffer.take().unwrap();
    if buffer.len() > MAX_BLOCK_SIZE {
      self.current_buffer = Some(buffer.split_off(MAX_BLOCK_SIZE));
    }

    match self.encode_frame(buffer) {
      Err(e) => Err(io::Error::new(io::ErrorKind::InvalidInput, e)),
      Ok(out) => Ok(Async::Ready(Some(out)))
    }
  }
}


fn crc32c_masked(buf: &[u8]) -> u32 {
  let sum = crc32::checksum_castagnoli(buf);
  (sum.wrapping_shr(15) | sum.wrapping_shl(17)).wrapping_add(0xa282ead8)
}
