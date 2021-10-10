#![allow(unused_imports)]

use bytes::{ Bytes, BytesMut};
use mio::unix::pipe::new;
use snow::types::Hash;
use time::error;
use std::{net::SocketAddr, str::from_utf8};
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;
use tokio::sync::{
  broadcast,
  mpsc,
};
use futures::SinkExt;
use anyhow::{Result, bail, anyhow};
use clap::App;
use tracing;
pub mod tokio_netstring;
use tokio_netstring::*;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
struct CommChannel {
  sender: tokio::sync::broadcast::Sender<UplinkEvent>
}

impl CommChannel {
  pub fn new() -> Self {
    let (sender, _): (tokio::sync::broadcast::Sender<UplinkEvent>,_)  = tokio::sync::broadcast::channel(64);
    CommChannel {
      sender,
    }
  }
}
#[derive(Clone, Debug)]
enum UplinkEvent {
  MessageReceived(MessageType),
  MessageProduced(MessageType),
  Heartbeat,
  ConnectionBroken,
  Void,
}

struct NetUser {
  transport: Framed<TcpStream, NetstringCodec>,
}

impl NetUser {
  async fn await_message(&mut self) -> anyhow::Result<MessageType> {
    let minwait : usize = 10;
    let maxwait : usize = 250;
    let mut waittime : usize = minwait; //msec
    loop {
      let opt = self.transport.next().await;
      match opt {
        Some(res) => {
          waittime = minwait;
          tracing::info!("got something");
          match res {
            Ok(msg) => {
              tracing::info!("got proper message");
              return Ok(msg)
            }
            Err(e) => {
              tracing::info!("got error{}", e);
              bail!(e)
            }
          }
        }
        None => {
          if waittime < maxwait { waittime = waittime+10 }
          tracing::info!("got nothing, wait {}", waittime);
          tokio::time::sleep(Duration::from_millis(waittime as u64)).await;
        }
      }
    }
    // while let Some(message) =  self.transport.next().await {
    //   return message.map_err(|e| anyhow::Error::new(e) )
    // };
    // bail! ("connection broken?");
  }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let matches = App::new("simple").args_from_usage("-s --server 'Server mode'").get_matches();
  let addr = "127.0.0.1:20000".to_string();
  tracing_subscriber::fmt::init();
  tracing::info!("started");
  if matches.is_present("server") {
    let socket = TcpListener::bind(&addr).await?;
    let mut commchannel = CommChannel::new();
    loop {
      tracing::info!("new connection");
      let (connection, clientaddr) = socket.accept().await?;
      let channel = commchannel.sender.clone();
      tokio::spawn(async move {
        handle_connection(connection, clientaddr,  channel ).await;
      });
    }
  } else {
    let connection = TcpStream::connect(&addr).await?;
    let mut transport = tokio_util::codec::Framed::new(connection, tokio_netstring::NetstringCodec);
    // transport.send(MessageType::Message { payload : { BytesMut::new().extend_from_slice(b"Hello World"), }).await?;
    transport.send(newStandardMessage_string("HelloWorld".to_owned()).await).await;
    let out = transport.next().await;
    tracing::info!("got {:?}", out);
  }

  Ok(())
}

// async fn server_process(transport: &mut Framed<TcpStream, NetstringCodec>, m: &Bytes) -> Result<()> {
//   let mut buf = BytesMut::with_capacity(m.len() + 20);
//   buf.extend_from_slice(&m);
//   buf.extend_from_slice(b" <- got this, i am echoing!");
//   transport.send(buf.freeze()).await?;
//   Ok(())
// }

async fn heartbeat(milis: Option<u64>) -> UplinkEvent {
  if milis.is_none() {
    tokio::time::sleep(Duration::from_millis(1000)).await;
  } else {
    let mut time = milis.unwrap();
    if time < 1000 { time = 1000 };
    tokio::time::sleep(Duration::from_millis(time)).await;
  };
  UplinkEvent::Heartbeat
}

async fn handle_connection_io(mut netuser : NetUser, mut io_input: tokio::sync::broadcast::Receiver<UplinkEvent>, mut io_output: tokio::sync::broadcast::Sender<UplinkEvent>) -> anyhow::Result<()> {
  loop {
    let out = tokio::select! {
      message = netuser.await_message() => {
        if let Ok(data) = message {
          UplinkEvent::MessageReceived(data)
        } else { 
          UplinkEvent::ConnectionBroken 
        }
      }
      event = heartbeat(None) => {
        event
      }
      tosend = io_input.recv() => {
        match tosend {
          Ok(x) => x,
          Err(e) => {
            UplinkEvent::Void
          } 
        }
      }
    };
    match out {
      UplinkEvent::Heartbeat => {
        netuser.transport.send(MessageType::Heartbeat).await?
      }
      UplinkEvent::ConnectionBroken  => {
        tracing::info!("breaking connection");
        break
      }
      UplinkEvent::MessageProduced(m) => {
        tracing::info!("sending shit");
        netuser.transport.send(m).await? // write motherfucker as is
      }
      UplinkEvent::MessageReceived(m) => {
        tracing::info!("sending shit");
        netuser.transport.send(m).await? // just echo for now //todo
        // todo io_output.send() // this shit into orbit
      }
      _ => {
        tracing::info!("shit happened at io controller");
      }
    }
  }
  Ok(())
}


// get stream,
// create transport
// spawn io driver for user
// join default (only) channel which is basically a broadcast
// they saw me rolling, thay hated
async fn handle_connection(connection: TcpStream, connection_address: SocketAddr, commchannel : tokio::sync::broadcast::Sender<UplinkEvent>) {
  tracing::info!("conenction from {}, handling", connection_address);
  let mut netuser =  NetUser {
    transport: tokio_util::codec::Framed::new(connection, tokio_netstring::NetstringCodec),
  };

  
  tokio::spawn(async move {
    handle_connection_io( netuser, commchannel.subscribe(), commchannel.clone()).await
  });
  // tokio::spawn(async move {
  //   handle_connection(connection, clientaddr,  channel ).await;
  // });

  // loop {

  //   match out {
  //     UplinkEvent::MessageReceived(x) => {
  //       server_process(tra) // todo write driver
  //     }
  //   }
    // if let Ok(message) = netuser.await_message().await {
    //   // match server_process(transport, m)
    // }
    // while let Some(res) = transport.next().await {
    //   match res {
    //     Err(e) => tracing::error!("got error {}", e),
    //     Ok(m) => {
    //       match server_process(&mut transport, &m.freeze()).await {
    //         Ok(()) => (),
    //         Err(e) => tracing::error!("error encountered {}", e),
    //       }
    //     }
    //   }
    // };
  // }
}

#[test]
fn message_encode() -> anyhow::Result<()> {
  fn newStandardMessage_string_sync(s: String) -> MessageType {
    let mut out = BytesMut::with_capacity(s.len());
    out.extend_from_slice(s.as_bytes());
    return MessageType::Message{payload: out}
}

  use tokio_util::codec::Encoder;
  use bytes::BufMut;

  let datastring = "data".to_owned();
  let data = datastring.as_bytes();
  let mut netstring_codec = NetstringCodec;
  let mut output = BytesMut::new();
  let mut message = bytes::BytesMut::new();
  // message.extend_from_slice(data);
  netstring_codec.encode(newStandardMessage_string_sync(datastring.clone()), &mut output)?;

  let mut expect = bytes::BytesMut::new();
  expect.put_u32_le(data.len() as u32);
  expect.put_u8(UPLINK_MSGT_MESSAGE);
  expect.extend_from_slice(data);

  assert_eq!(
    output,
    expect,
  );
  Ok(())
}