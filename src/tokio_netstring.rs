#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]
use tokio_util::codec::Decoder;
use tokio_util::codec::Encoder;

use futures::StreamExt;

use bytes::{BufMut, Bytes, BytesMut};
use std::io;
use std::str;
const MAX: usize = 8 * 1024 * 1024;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct NetstringCodec;

impl Decoder for NetstringCodec {
    type Item = BytesMut;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<BytesMut>, io::Error> {
        if src.len() < 4 {
            return Ok(None);
        }

        let mut length_bytes = [0u8; 4];
        length_bytes.copy_from_slice(&src[..4]);
        let length = u32::from_le_bytes(length_bytes) as usize;

        if length > MAX {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Frame of length {} is too large.", length)
            ));
        }

        if src.len() < 4 + length {
            src.reserve(4 + length - src.len());
            return Ok(None);
        }

        let out = src.split_to(4);
        let out = src.split_to(length);
        tracing::info!("dcode {{ size={}, data={} }}", length, str::from_utf8(&src).unwrap());
        Ok(Some(out))
    }
}


impl Encoder<Bytes> for NetstringCodec {
    type Error = std::io::Error;

    fn encode(&mut self, item: Bytes, dst: &mut BytesMut) -> Result<(), Self::Error> {

        if item.len() > MAX {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Frame of length {} is too large.", item.len())
            ));
        }

        let len_slice = u32::to_le_bytes(item.len() as u32);
        dst.reserve(4 + item.len());

        dst.extend_from_slice(&len_slice);
        dst.extend_from_slice(&item);
        tracing::info!("encode {{ size={}, data={} }}", item.len(), str::from_utf8(&item).unwrap());
        Ok(())
    }
}

impl Encoder<String> for NetstringCodec {
    type Error = std::io::Error;

    fn encode(&mut self, item: String, dst: &mut BytesMut) -> Result<(), Self::Error> {
        if item.len() > MAX {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Frame of length {} is too large.", item.len())
            ));
        }
        let len_slice = u32::to_le_bytes(item.len() as u32);
        dst.reserve(4 + item.len());

        dst.extend_from_slice(&len_slice);
        dst.extend_from_slice(&item.as_bytes());
        tracing::info!("encode {{ size={}, data={} }}", item.len(), item);
        Ok(())
    }
}