#![feature(let_else)]

use std::future::Future;
use bytes::{Bytes, BytesMut};
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::StreamExt as _;
use tokio_util::codec::{Encoder, Framed};
use futures::{AsyncReadExt, Sink, SinkExt, StreamExt};
use anyhow::Result;
use clap::App;
use futures::stream::{SplitSink, SplitStream};
use tokio::io::AsyncWriteExt;
use tracing;
pub mod tokio_netstring;
use tokio_netstring::NetstringCodec;
use tokio_stream::Stream;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let matches = App::new("simple").args_from_usage("-s --server 'Server mode'").get_matches();
    let addr = "127.0.0.1:20000".to_string();

    if matches.is_present("server") {
        let socket = TcpListener::bind(&addr).await?;
        loop {
            let (connection, clientaddr) = socket.accept().await?;
            tokio::spawn(async move {
                handle_connection(connection, clientaddr).await;
            });
        }
    } else {
        let connection = TcpStream::connect(&addr).await?;
        let mut transport = tokio_util::codec::Framed::new(connection, tokio_netstring::NetstringCodec);
        transport.send(bytes::Bytes::from("Hello World")).await?;
    }

    Ok(())
}

fn server_process(transport: Pin<&mut SplitSink<Framed<TcpStream, NetstringCodec>, Bytes>>, m: &Bytes, cx: &mut Context<'_>) -> Result<()> {
    let mut buf = BytesMut::with_capacity(m.len() + 20);
    buf.extend_from_slice(&m);
    buf.extend_from_slice(b" <- got this, i am echoing!");
    transport.start_send(buf.freeze()).unwrap();
    Ok(())
}

struct Receiver {
    // frame: Framed<TcpStream, NetstringCodec>,
    sink: SplitSink<Framed<TcpStream, NetstringCodec>, Bytes>,
    stream: SplitStream<Framed<TcpStream, NetstringCodec>>,
}

impl Future for Receiver {
    type Output = Result<BytesMut, std::io::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        while let Poll::Ready(res) = Pin::new(&mut self.stream).poll_next(cx) {
            let Some(res) = res else { println!("res None"); break };
            let Ok(x) = res else { println!("res Err"); break };
            eprintln!("received {:?}", x);
            server_process(Pin::new(&mut self.sink), &x.freeze(), cx);
        }
        Poll::Pending
    }
}

async fn handle_connection(connection: TcpStream, connection_address: SocketAddr) {
    tracing::info!("conenction from {}, handling", connection_address);
    let frame = tokio_util::codec::Framed::new(connection, tokio_netstring::NetstringCodec);
    let (sink, stream) = frame.split();
    eprintln!("split");
    tokio::spawn(Receiver { sink, stream }).await;
}

#[test]
fn message_encode() -> anyhow::Result<()> {
    use tokio_util::codec::Encoder;
    use bytes::BufMut;

    let data = "data".as_bytes();
    let mut netstring_codec = NetstringCodec;
    let mut output = BytesMut::new();
    let mut message = bytes::BytesMut::new();
    message.extend_from_slice(data);
    netstring_codec.encode(message.freeze(), &mut output)?;

    let mut expect = bytes::BytesMut::new();
    expect.put_u32_le(4);
    expect.extend_from_slice(data);

    assert_eq!(
        output,
        expect,
    );
    Ok(())
}
