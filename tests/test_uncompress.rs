extern crate bytes;
extern crate futures;
extern crate gingersnap;

#[cfg(test)]
mod test_uncompress {
  use bytes::{Bytes};
  use futures::{Future, stream};
  use gingersnap::{ByteStream, SnappyUncompress};
  use std::io;

  static HEADER: &str = "ff060000734e61507059";

  #[test]
  fn small_data() {
    // 9-byte uncompressed frame
    let s = from_hex(format!("{}{}{}{}", HEADER, "01090000", "bb1f1c19", "68656c6c6f"));
    let sc = SnappyUncompress::new(s);
    assert_eq!(to_hex(sc), "68656c6c6f");
  }

  #[test]
  fn compressed() {
    // 10-byte compressed frame
    let s = from_hex(format!("{}{}{}{}", HEADER, "000a0000", "59772563", "1800395a0100"));
    let sc = SnappyUncompress::new(s);
    assert_eq!(to_hex(sc), "393939393939393939393939393939393939393939393939");
  }


  fn to_hex<S: ByteStream>(s: S) -> String {
    let buffers: Vec<Bytes> = s.collect().wait().unwrap();
    let strings: Vec<String> = buffers.iter().map(|buffer| {
      buffer.as_ref().iter().map(|b| format!("{:02x}", b)).collect()
    }).collect();
    strings.join("")
  }

  fn from_hex(s: String) -> stream::Once<Bytes, io::Error> {
    // rust still doesn't have step_by! :(
    let bytes: Vec<u8> = (0 .. s.len() / 2).map(|i| {
      u8::from_str_radix(&s[i * 2 .. (i + 1) * 2], 16).unwrap()
    }).collect();
    stream::once(Ok(Bytes::from(bytes)))
  }
}
