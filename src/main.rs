use bytes::{ Bytes, BytesMut};
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;
use futures::SinkExt;
use anyhow::Result;
use clap::App;
use tracing;
pub mod tokio_netstring;
use tokio_netstring::NetstringCodec;

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

async fn server_process(transport: &mut Framed<TcpStream, NetstringCodec>, m: &Bytes) -> Result<()> {
    let mut buf = BytesMut::with_capacity(m.len() + 20);
    buf.extend_from_slice(&m);
    buf.extend_from_slice(b" <- got this, i am echoing!");
    transport.send(buf.freeze()).await?;
    Ok(())
}

async fn handle_connection(connection: TcpStream, connection_address: SocketAddr) {
    tracing::info!("conenction from {}, handling", connection_address);
    let mut transport = tokio_util::codec::Framed::new(connection, tokio_netstring::NetstringCodec);
    loop {
        while let Some(res) = transport.next().await {
            match res {
                Err(e) => tracing::error!("got error {}", e),
                Ok(m) => {
                    match server_process(&mut transport, &m.freeze()).await {
                        Ok(()) => (),
                        Err(e) => tracing::error!("error encountered {}", e),
                    }
                }
            }
        };
    }
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