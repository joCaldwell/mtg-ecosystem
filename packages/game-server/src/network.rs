//! Loopback transport bounds. Connection slots include idle HTTP keep-alives;
//! no unbounded accept tasks wait for an application request permit.
//! Listener API: axum0.8.9/src/serve/listener.rs; Tokio AsyncRead/AsyncWrite.
use axum::serve::Listener;
use std::{
    io,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::{OwnedSemaphorePermit, Semaphore},
    time::{Instant, Sleep},
};

pub const CONNECTION_LIMIT: usize = 64;
const IDLE_LIMIT: Duration = Duration::from_secs(30);

pub struct LimitedListener {
    listener: TcpListener,
    slots: Arc<Semaphore>,
}
impl LimitedListener {
    pub fn new(listener: TcpListener) -> io::Result<Self> {
        if !listener.local_addr()?.ip().is_loopback() {
            return Err(io::Error::other("only loopback hosting is supported"));
        }
        Ok(Self {
            listener,
            slots: Arc::new(Semaphore::new(CONNECTION_LIMIT)),
        })
    }
}
pub struct LimitedStream {
    stream: TcpStream,
    _slot: OwnedSemaphorePermit,
    idle: Pin<Box<Sleep>>,
}
impl LimitedStream {
    fn check_idle(&mut self, cx: &mut Context<'_>) -> io::Result<()> {
        if std::future::Future::poll(self.idle.as_mut(), cx).is_ready() {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "connection idle limit",
            ))
        } else {
            Ok(())
        }
    }
    fn activity(&mut self) {
        self.idle.as_mut().reset(Instant::now() + IDLE_LIMIT);
    }
}
impl AsyncRead for LimitedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if let Err(error) = self.check_idle(cx) {
            return Poll::Ready(Err(error));
        }
        let old = buf.filled().len();
        let result = Pin::new(&mut self.stream).poll_read(cx, buf);
        if matches!(result, Poll::Ready(Ok(()))) && buf.filled().len() > old {
            self.activity();
        }
        result
    }
}
impl AsyncWrite for LimitedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if let Err(error) = self.check_idle(cx) {
            return Poll::Ready(Err(error));
        }
        let result = Pin::new(&mut self.stream).poll_write(cx, buf);
        if matches!(result, Poll::Ready(Ok(n)) if n > 0) {
            self.activity();
        }
        result
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}
impl Listener for LimitedListener {
    type Io = LimitedStream;
    type Addr = SocketAddr;
    async fn accept(&mut self) -> (Self::Io, SocketAddr) {
        // Acquire before accept: bounded kernel backlog, no per-connection task
        // is spawned until its lifetime permit is available.
        let slot = self
            .slots
            .clone()
            .acquire_owned()
            .await
            .expect("listener owns semaphore");
        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    return (
                        LimitedStream {
                            stream,
                            _slot: slot,
                            idle: Box::pin(tokio::time::sleep(IDLE_LIMIT)),
                        },
                        addr,
                    );
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
    }
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;
    #[tokio::test]
    async fn connection_lifetime_slots_bound_accept_and_release_on_drop() {
        let socket = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();
        let mut listener = LimitedListener::new(socket).unwrap();
        let mut clients = Vec::new();
        let mut accepted = Vec::new();
        for _ in 0..CONNECTION_LIMIT {
            clients.push(TcpStream::connect(address).await.unwrap());
            accepted.push(listener.accept().await.0);
        }
        clients.push(TcpStream::connect(address).await.unwrap());
        assert!(
            tokio::time::timeout(Duration::from_millis(20), listener.accept())
                .await
                .is_err()
        );
        accepted.pop();
        let (stream, _) = tokio::time::timeout(Duration::from_secs(1), listener.accept())
            .await
            .unwrap();
        assert_eq!(listener.slots.available_permits(), 0);
        drop(stream);
        assert_eq!(listener.slots.available_permits(), 1);
    }
    #[tokio::test]
    async fn idle_connection_cannot_retain_a_slot_forever() {
        let socket = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = socket.local_addr().unwrap();
        let mut listener = LimitedListener::new(socket).unwrap();
        let _client = TcpStream::connect(address).await.unwrap();
        let (mut stream, _) = listener.accept().await;
        stream
            .idle
            .as_mut()
            .reset(Instant::now() + Duration::from_millis(20));
        let error = stream.read_u8().await.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }
}
