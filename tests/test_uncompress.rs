extern crate bytes;
extern crate futures;
extern crate gingersnap;

#[cfg(test)]
mod test_uncompress {
  use bytes::{Bytes};
  use futures::{Future, stream};
  use gingersnap::{ByteStream, SnappyUncompress};
  use std::io;
  use std::vec;

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

  #[test]
  fn coalesce_small_chunks() {
    let s = from_hexes(vec![ &HEADER[0..2], &HEADER[2..6], &HEADER[6..], "000a00", "005977", "2563180039", "5a0100" ]);
    let sc = SnappyUncompress::new(s);
    assert_eq!(to_hex(sc), "393939393939393939393939393939393939393939393939");
  }

  #[test]
  #[should_panic(expected="Truncated snappy frame")]
  fn truncated_header() {
    let s = from_hexes(vec![ &HEADER[0..2] ]);
    let sc = SnappyUncompress::new(s);
    to_hex(sc);
  }

  #[test]
  fn full_header() {
    let s = from_hexes(vec![ HEADER ]);
    let sc = SnappyUncompress::new(s);
    assert_eq!(to_hex(sc), "");
  }

  #[test]
  #[should_panic(expected="Truncated snappy frame")]
  fn truncated_frame() {
    let s = from_hexes(vec![ HEADER, "000a00" ]);
    let sc = SnappyUncompress::new(s);
    to_hex(sc);
  }

  #[test]
  #[should_panic(expected="Truncated snappy frame")]
  fn truncated_frame_body() {
    let s = from_hexes(vec![ HEADER, "000a0000", "59772563", "1800395" ]);
    let sc = SnappyUncompress::new(s);
    to_hex(sc);
  }

  #[test]
  #[should_panic(expected="CRC mismatch")]
  fn wrong_crc() {
    let s = from_hexes(vec![ HEADER, "000a0000", "ff772563", "1800395a0100" ]);
    let sc = SnappyUncompress::new(s);
    to_hex(sc);
  }

  #[test]
  #[should_panic(expected="missing magic")]
  fn missing_magic() {
    let s = from_hexes(vec![ "000a0000", "ff772563", "1800395a0100" ]);
    let sc = SnappyUncompress::new(s);
    to_hex(sc);
  }

  #[test]
  #[should_panic(expected="mangled magic")]
  fn mangled_magic() {
    let s = from_hexes(vec![ "ff060000734e41505059", "000a0000", "ff772563", "1800395a0100" ]);
    let sc = SnappyUncompress::new(s);
    to_hex(sc);
  }

  #[test]
  #[should_panic(expected="Unknown frame type")]
  fn unknown_frame_type() {
    let s = from_hexes(vec![ HEADER, "030a0000", "ff772563", "1800395a0100" ]);
    let sc = SnappyUncompress::new(s);
    to_hex(sc);
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

  fn from_hexes(vec: Vec<&str>) -> stream::IterStream<vec::IntoIter<Result<Bytes, io::Error>>> {
    let bytes_vec: Vec<Result<Bytes, io::Error>> = vec.iter().map(|s| {
      // rust still doesn't have step_by! :(
      let bytes: Vec<u8> = (0 .. s.len() / 2).map(|i| {
        u8::from_str_radix(&s[i * 2 .. (i + 1) * 2], 16).unwrap()
      }).collect();
      Ok(Bytes::from(bytes))
    }).collect();
    stream::iter(bytes_vec.into_iter())
  }
}
