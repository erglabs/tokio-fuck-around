#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]
use bytes::Buf;
use tokio_util::codec::Decoder;
use tokio_util::codec::Encoder;

use futures::StreamExt;

use bytes::{BufMut, Bytes, BytesMut};
use std::io;
use std::io::Read;
use std::str;
const UPLINK_MAXPAYLOADSIZE: usize = 8 * 1024 * 1024;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct NetstringCodec;

pub static UPLINK_HEADERSIZE :usize =  5;
pub static UPLINK_MARKERSIZE :usize =  1;
pub static UPLINK_LENGTHSIZE :usize =  4;
pub static UPLINK_MSGT_HEARTBEAT : u8 = 0;
pub static UPLINK_MSGT_CONTROL   : u8 = 1;
pub static UPLINK_MSGT_MESSAGE   : u8 = 2;
pub static UPLINK_MSGT_TRANSFER  : u8 = 3;
pub static UPLINK_MSGT_VOID      : u8 = 7;

//-----------------------------------
//| --- 32 --- | -- 8 -- |-- ... -- | 
//|   length   |   type  | payload  |
//-----------------------------------


// not used yet todo:
pub enum ControlType {
    BreakOff,
}
  
#[derive(Clone, Debug)]
pub enum MessageType {        // u8 marker
    Heartbeat,              // 0
    Control,                // 1
    Message {               // 2
        payload : BytesMut,
    },      
    // Transfer,            // 3
    Void,                   // 7
  }

pub async fn newStandardMessage_string(s: String) -> MessageType {
    let mut out = BytesMut::with_capacity(s.len());
    out.extend_from_slice(s.as_bytes());
    return MessageType::Message{payload: out}
}
impl Decoder for NetstringCodec {
    type Item = MessageType;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<MessageType>, io::Error> {
        if src.len() < UPLINK_HEADERSIZE { // not received enough
            return Ok(None);  // break off without mutation
        }
        tracing::info!("netstring::decode::src_content={:?}", src);
        tracing::info!("netstring::decode::src_len={}", src.len());
        // let mut header = src.copy_to_bytes(UPLINK_HEADERSIZE);
        // copy and check the buffer length
        let length = src.get_u32_le();
        let marker = src.get_u8();
        tracing::info!("netstring::decode::length={} ", length);
        tracing::info!("netstring::decode::marker={}", marker);
        tracing::info!("netstring::decode::src_len={}", src.len());

        if length as usize > UPLINK_MAXPAYLOADSIZE {
            tracing::info!("netstring::decode::toolarge");
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Frame of length {} is too large.", length),
            ));
        } // we need to kill connection here, malformed shit in buffer

        if length == 0 || marker == 0 {
            tracing::info!("netstring::decode::got_heartbeat");
            // heartbeat incoming, drop header, create empty message type
            return Ok(Some(MessageType::Heartbeat));
        } // we got mail

        // not enought data yet, lets skip the rest
        if src.len() < (length as usize) as usize { 
            tracing::info!("netstring::decode::notenough is {} wants {}", src.len(), UPLINK_HEADERSIZE + length as usize);
            return Ok(None);
        }

        // shit seems okay, lets create it properly
        // drop(src.split_to(UPLINK_HEADERSIZE)); // become ungovernable
        tracing::info!("netstring::decode::matching?");
        tracing::info!("netstring::decode::src_len={}", src.len());
        // drop(src.split_to(UPLINK_HEADERSIZE));
        match marker {
            1 => {
                tracing::info!("netstring::decode::got:UPLINK_MSGT_CONTROL"); 
                return Ok(Some(MessageType::Control)) 
            }
            2 => {
                tracing::info!("netstring::decode::got:UPLINK_MSGT_MESSAGE, len={}", length);  
                return Ok(Some(MessageType::Message{ payload: src.split_to(length as usize)}))
            },
            // 3 => todo implement transfers
            _ => {
                tracing::info!("netstring::decode::got:UPLINK_MSGT_VOID");
                return Ok(Some(MessageType::Void))
            },
        };
    }
}

impl Encoder<MessageType> for NetstringCodec {
    type Error = std::io::Error;

    fn encode(&mut self, item: MessageType, dst: &mut BytesMut) -> Result<(), Self::Error> {
        
        match item {
            MessageType::Heartbeat => {
                tracing::info!("netstring::encode::heartbeat::sending");
                dst.put_u32_le(0);
                dst.put_u8(0);
                tracing::info!("netstring::encode::heartbeat::dst_len={}", dst.len());
                return Ok(())
            },
            MessageType::Control => {
                // lets mask it as heartbeat for now, fuck it
                dst.put_u32_le(0);
                dst.put_u8(0);
                tracing::info!("netstring::encode::Control::dst_len={}", dst.len());
                tracing::info!("netstring::encode::control::"); // todo
                todo!();
            }
            MessageType::Message{ payload: msg} => {
                if msg.len() > UPLINK_MAXPAYLOADSIZE {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Frame of length {} is too large.", msg.len()),
                    ));
                }
                dst.put_u32_le(msg.len() as u32);
                dst.put_u8(2);
                dst.extend_from_slice(&msg);
                tracing::info!("netstring::encode::message::dst_len={}", dst.len());
                tracing::info!("netstring::encode::encode {{ size={}, data={} }}", msg.len(), str::from_utf8(&msg).unwrap());
            }
            _ => ()
        }
        Ok(())
    }
}
