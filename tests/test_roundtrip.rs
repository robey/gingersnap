extern crate bytes;
extern crate futures;
extern crate gingersnap;

#[cfg(test)]
mod test_roundtrip {
  use bytes::{Bytes};
  use futures::{Async, Poll, Future, Stream, stream};
  use gingersnap::{SnappyCompress, SnappyUncompress};
  use std::fs;
  use std::io;
  use std::io::Read;

  #[test]
  fn alice29() {
    let s = stream_from_file("./data/alice29.txt").unwrap();
    let sc = SnappyCompress::new(s);
    let compressed = sc.collect().wait().unwrap();
    let compressed_bytes = compressed.iter().fold(0, |sum, b| sum + b.len());

    let s2 = stream::iter(compressed.into_iter().map(|b| Ok(b)));
    let su = SnappyUncompress::new(s2);
    let uncompressed = su.collect().wait().unwrap();
    let uncompressed_bytes = uncompressed.iter().fold(0, |sum, b| sum + b.len());

    let original = stream_from_file("./data/alice29.txt").unwrap().collect().wait().unwrap();
    let original_bytes = original.iter().fold(0, |sum, b| sum + b.len());

    assert_eq!(original_bytes, uncompressed_bytes);
    assert!(compressed_bytes < original_bytes);
    for (a, b) in original.iter().zip(uncompressed.iter()) {
      assert_eq!(a, b);
    }
  }

  fn stream_from_file(filename: &str) -> Result<FileReadStream, io::Error> {
    let file = fs::File::open(filename)?;
    Ok(FileReadStream::new(file))
  }

  // use a small buffer size to force the stream to generate lots of blocks :)
  const BUFFER_SIZE: usize = 8192;

  pub struct FileReadStream {
    file: fs::File,
    buffer: Vec<u8>,
  }

  impl FileReadStream {
    pub fn new(file: fs::File) -> FileReadStream {
      let mut s = FileReadStream {
        file,
        buffer: Vec::with_capacity(BUFFER_SIZE),
      };
      s.buffer.resize(BUFFER_SIZE, 0);
      s
    }
  }

  impl Stream for FileReadStream {
    type Item = Bytes;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
      match self.file.read(self.buffer.as_mut()) {
        Err(e) => Err(e),
        Ok(length) => {
          if length == 0 {
            Ok(Async::Ready(None))
          } else {
            Ok(Async::Ready(Some(Bytes::from(&self.buffer[..length]))))
          }
        }
      }
    }
  }
}
