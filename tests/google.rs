use std::net::ToSocketAddrs;
use schannel::schannel_cred::{SchannelCred, Direction};
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

macro_rules! t {
    ($e:expr) => {
        match $e {
            Ok(e) => e,
            Err(e) => panic!("{} failed with {:?}", stringify!($e), e),
        }
    };
}

#[tokio::test]
async fn fetch_google() {
    // First up, resolve google.com
    let addr = t!("google.com:443".to_socket_addrs()).next().unwrap();

    let socket = TcpStream::connect(&addr).await.unwrap();

    let builder = SchannelCred::builder();
    let cred = t!(builder.acquire(Direction::Outbound));

    let mut builder = schannel::tls_stream::Builder::new();
    builder
        .domain("google.com");
    
    let mut connector = tokio_schannel::Builder::from(builder);
    let mut socket = t!(connector.connect(cred, socket).await);
    t!(socket.write_all(b"GET / HTTP/1.0\r\n\r\n").await);
    let mut data = Vec::new();
    t!(socket.read_to_end(&mut data).await);

    // any response code is fine
    assert!(data.starts_with(b"HTTP/1.0 "));

    let data = String::from_utf8_lossy(&data);
    let data = data.trim_end();
    assert!(data.ends_with("</html>") || data.ends_with("</HTML>"));
}