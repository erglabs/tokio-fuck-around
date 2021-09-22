use anyhow::{Context, Result};
use bytes::{Bytes, BytesMut};
use clap::App;
use futures::StreamExt;
use futures::{Future, FutureExt, SinkExt};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::wrappers::IntervalStream;
use tokio_util::codec::Framed;

pub mod tokio_netstring;
use tokio_netstring::NetstringCodec;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let matches = App::new("simple")
        .args_from_usage("-s --server 'Server mode'")
        .get_matches();
    let addr = "127.0.0.1:20000".to_string();

    if matches.is_present("server") {
        let socket = TcpListener::bind(&addr).await?;
        loop {
            let (connection, clientaddr) = socket.accept().await?;
            // let mut transport = tokio_util::codec::Framed::new(connection, tokio_netstring::NetstringCodec);
            tokio::spawn(async move {
                handle_connection(connection, clientaddr).await;
            });
        }
    } else {
        let connection = TcpStream::connect(&addr).await?;
        let mut transport =
            tokio_util::codec::Framed::new(connection, tokio_netstring::NetstringCodec);
        transport.send(bytes::Bytes::from("Hello World")).await?;
    }

    Ok(())
}

async fn server_process(
    transport: &mut Framed<TcpStream, NetstringCodec>,
    m: &Bytes,
) -> Result<()> {
    let mut buf = BytesMut::with_capacity(m.len() + 20);
    buf.extend_from_slice(&m);
    buf.extend_from_slice(b" <- got this, i am echoing!");
    transport.send(buf.freeze()).await?;
    Ok(())
}

async fn handle_connection(connection: TcpStream, connection_address: SocketAddr) {
    enum ConnectionEvent {
        Heartbeat,
        NetworkEvent(BytesMut),
        NetworkError(anyhow::Error),
    }

    // transport (getting and sending at diff. times)
    //
    // Ingest
    // XXXX ____ XXXX ____
    // Send
    // ____ XXXX ____ ____

    println!("connection from {}, handling", connection_address);
    let mut transport = tokio_util::codec::Framed::new(connection, tokio_netstring::NetstringCodec);
    let (mut sink, mut stream): (tokio_util::codec::Fram) = transport.split();
    let mut events = stream.map(|res| match res {
        Ok(event) => ConnectionEvent::NetworkEvent(event),
        Err(err) => {
            ConnectionEvent::NetworkError(anyhow::anyhow!("Network error occurred: {}", err))
        }
    });
    let heartbeat = IntervalStream::new(tokio::time::interval(Duration::from_secs(60)))
        .map(|_| ConnectionEvent::Heartbeat);

    let merged_events = tokio_stream::StreamExt::merge(events, heartbeat);

    loop {
        while let Some(res) = merged_events.next().await {
            match res {
                ConnectionEvent::NetworkEvent(event) => {
                    match server_process(&mut transport, &event.freeze()).await {
                        Ok(()) => (),
                        Err(e) => eprintln!("error encountered {}", e),
                    }
                }
                ConnectionEvent::NetworkError(error) => eprintln!("got error: {}", error),
                ConnectionEvent::Heartbeat => {
                    match server_process(&mut transport, &Default::default()).await {
                        Ok(()) => (),
                        Err(e) => eprintln!("error encountered {}", e),
                    }
                }
            }
        }
    }
}

#[test]
fn message_encode() -> anyhow::Result<()> {
    use bytes::BufMut;
    use tokio_util::codec::Encoder;

    let data = "data".as_bytes();
    let mut netstring_codec = NetstringCodec;
    let mut output = BytesMut::new();
    let mut message = bytes::BytesMut::new();
    message.extend_from_slice(data);
    netstring_codec.encode(message.freeze(), &mut output)?;

    let mut expect = bytes::BytesMut::new();
    expect.put_u32_le(4);
    expect.extend_from_slice(data);

    assert_eq!(output, expect,);
    Ok(())
}
