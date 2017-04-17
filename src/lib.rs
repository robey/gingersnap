#![feature(try_from)]

extern crate bytes;
extern crate crc;
extern crate futures;
extern crate snap;

#[macro_use]
extern crate lazy_static;

pub mod aliases;
pub mod compress;
pub mod shared;
pub mod uncompress;

pub use aliases::{ByteStream};
pub use compress::{SnappyCompress};
pub use uncompress::{SnappyUncompress};
