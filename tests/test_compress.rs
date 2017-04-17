extern crate bytes;
extern crate futures;
extern crate gingersnap;

#[cfg(test)]
mod test_compress {
  use bytes::{BufMut, Bytes, BytesMut};
  use futures::{Future, stream};
  use gingersnap::{ByteStream, SnappyCompress};

  static HEADER: &str = "ff060000734e61507059";

  #[test]
  fn small_data() {
    let s = stream::once(Ok(Bytes::from(&b"hello"[..])));
    let sc = SnappyCompress::new(s);
    // should just be a 9-byte uncompressed frame
    assert_eq!(to_hex(sc), format!("{}{}{}{}", HEADER, "01090000", "bb1f1c19", "68656c6c6f"));
  }

  #[test]
  fn compressable() {
    let s = stream::once(Ok(Bytes::from(&b"999999999999999999999999"[..])));
    let sc = SnappyCompress::new(s);
    // should be a 10-byte compressed frame!
    assert_eq!(to_hex(sc), format!("{}{}{}{}", HEADER, "000a0000", "59772563", "1800395a0100"));
  }

  #[test]
  fn multi_frame() {
    let mut crap = BytesMut::with_capacity(32);
    for _ in 0..32 { crap.put(0 as u8) };
    let b = crap.freeze();
    let s = stream::iter(vec![ Ok(b.clone()), Ok(b.clone()), Ok(b.clone()) ]);
    let sc = SnappyCompress::new(s);
    let frame = format!("{}{}{}", "000a0000", "faffd70f", "2000007a0100");
    assert_eq!(to_hex(sc), format!("{}{}{}{}", HEADER, frame, frame, frame));
  }


  fn to_hex<S: ByteStream>(s: S) -> String {
    let buffers: Vec<Bytes> = s.collect().wait().unwrap();
    let strings: Vec<String> = buffers.iter().map(|buffer| {
      buffer.as_ref().iter().map(|b| format!("{:02x}", b)).collect()
    }).collect();
    strings.join("")
  }
}
