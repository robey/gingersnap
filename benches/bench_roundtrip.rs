#![feature(test)]

extern crate bytes;
extern crate futures;
extern crate gingersnap;
extern crate test;

#[cfg(test)]
mod test_roundtrip {
  use bytes::{Bytes};
  use futures::{Future, Stream, stream};
  use gingersnap::{SnappyCompress, SnappyUncompress};
  use std::fs;
  use std::io::Read;
  use test::Bencher;

  const BUFFER_SIZE: usize = 65536;

  #[bench]
  fn bench_alice29_stream_roundtrip(b: &mut Bencher) {
    // read 64KB into a buffer.
    let mut file = fs::File::open("./data/alice29.txt").unwrap();
    let mut buffer: Vec<u8> = Vec::with_capacity(BUFFER_SIZE);
    buffer.resize(BUFFER_SIZE, 0);
    assert_eq!(file.read(buffer.as_mut()).unwrap(), BUFFER_SIZE);
    let data = Bytes::from(buffer);

    b.bytes = BUFFER_SIZE as u64;
    b.iter(|| {
      let s = stream::once(Ok(data.clone()));
      let sc = SnappyCompress::new(s);
      let su = SnappyUncompress::new(sc);
      su.collect().wait().unwrap();
    });
  }
}
