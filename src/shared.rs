use std::convert::TryFrom;
use crc::crc32;

// An enumeration describing each of the 4 main chunk types.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FrameType {
  Stream = 0xff,
  Compressed = 0x00,
  Uncompressed = 0x01,
  Padding = 0xfe,
}

impl TryFrom<u8> for FrameType {
  type Error = u8;

  fn try_from(b: u8) -> Result<FrameType, u8> {
    match b {
      0x00 => Ok(FrameType::Compressed),
      0x01 => Ok(FrameType::Uncompressed),
      0xfe => Ok(FrameType::Padding),
      0xff => Ok(FrameType::Stream),
      b => Err(b),
    }
  }
}

pub fn crc32c_masked(buf: &[u8]) -> u32 {
  let sum = crc32::checksum_castagnoli(buf);
  (sum.wrapping_shr(15) | sum.wrapping_shl(17)).wrapping_add(0xa282ead8)
}
