use crate::Error;
use axum_server::accept::Accept;
use socket2::{Socket, TcpKeepalive};
use std::future::Ready;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;
pub use tokio::net::TcpListener;
use tokio::task;
use tracing::debug;

fn standard_keepalive() -> TcpKeepalive {
    TcpKeepalive::new()
        .with_time(Duration::from_secs(60))
        .with_interval(Duration::from_secs(10))
        .with_retries(3)
}

pub async fn create_listening_socket(addr: SocketAddr) -> Result<TcpListener, Error> {
    task::spawn_blocking(move || {
        let socket = if addr.is_ipv4() {
            Socket::new(
                socket2::Domain::IPV4,
                socket2::Type::STREAM,
                Some(socket2::Protocol::TCP),
            )?
        } else {
            Socket::new(
                socket2::Domain::IPV6,
                socket2::Type::STREAM,
                Some(socket2::Protocol::TCP),
            )?
        };
        socket.set_reuse_address(true)?;
        socket.set_nonblocking(true)?;
        socket.set_tcp_nodelay(true)?;

        #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
        {
            socket.set_tcp_cork(false)?;
            socket.set_priority(6)?;
        }

        let keepalive = standard_keepalive();
        socket.set_tcp_keepalive(&keepalive)?;
        socket.set_keepalive(true)?;

        socket.bind(&addr.into())?;
        socket.listen(1024)?;
        Ok(TcpListener::from_std(socket.into())?)
    })
    .await?
}

pub fn configure_tcp_stream(stream: &mut tokio::net::TcpStream) {
    let socket_ref = socket2::SockRef::from(&*stream);

    if let Err(err) = stream.set_nodelay(true) {
        debug!("Failed to set TCP_NODELAY on TCP stream: {err}");
    }
    if let Err(err) = socket_ref.set_tcp_keepalive(&standard_keepalive()) {
        debug!("Failed to configure TCP keepalive on TCP stream: {err}");
    }
    #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
    if let Err(err) = socket_ref.set_priority(6) {
        debug!("Failed to set socket priority on TCP stream: {err}");
    }
}

#[derive(Clone, Copy)]
pub struct ConfiguredStreamAcceptor;

impl<S> Accept<tokio::net::TcpStream, S> for ConfiguredStreamAcceptor {
    type Stream = tokio::net::TcpStream;
    type Service = S;
    type Future = Ready<io::Result<(tokio::net::TcpStream, S)>>;

    fn accept(&self, stream: tokio::net::TcpStream, service: S) -> Self::Future {
        let mut stream = stream;
        configure_tcp_stream(&mut stream);
        std::future::ready(Ok((stream, service)))
    }
}
