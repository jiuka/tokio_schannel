use std::{fmt, io::{self, Read, Write}, future::Future, pin::Pin, task::{Context, Poll}, ptr::null_mut};

use schannel::{schannel_cred::SchannelCred, tls_stream::{HandshakeError, MidHandshakeTlsStream}};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{InnerBuilder, tls_stream::TlsStream, InnerTlsStream, allow_std::AllowStd};

/// A wrapper around a `schannel::tls_stream::Builder`, providing an async `connect`
/// and `accept` method.
pub struct Builder(InnerBuilder);

impl Builder {
    /// Connects the provided stream with this connector, assuming the provided
    /// domain.
    ///
    /// This function will internally call `TlsConnector::connect` to connect
    /// the stream and returns a future representing the resolution of the
    /// connection operation. The returned future will resolve to either
    /// `TlsStream<S>` or `Error` depending if it's successful or not.
    ///
    /// This is typically used for clients who have already established, for
    /// example, a TCP connection to a remote server. That stream is then
    /// provided here to perform the client half of a connection to a
    /// TLS-powered server.
    pub async fn connect<S>(&mut self, cred: SchannelCred, stream: S) -> Result<TlsStream<S>, io::Error>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        handshake(move |s| self.0.connect(cred, s), stream).await
    }

    /// Accepts a new client connection with the provided stream.
    ///
    /// This function will internally call `TlsAcceptor::accept` to connect
    /// the stream and returns a future representing the resolution of the
    /// connection operation. The returned future will resolve to either
    /// `TlsStream<S>` or `Error` depending if it's successful or not.
    ///
    /// This is typically used after a new socket has been accepted from a
    /// `TcpListener`. That socket is then passed to this function to perform
    /// the server half of accepting a client connection.
    pub async fn accept<S>(&mut self, cred: SchannelCred, stream: S) -> Result<TlsStream<S>, io::Error>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        handshake(move |s| self.0.accept(cred, s), stream).await
    }
}

impl fmt::Debug for Builder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Builder").finish()
    }
}

impl From<InnerBuilder> for Builder {
    fn from(inner: InnerBuilder) -> Builder {
        Builder(inner)
    }
}

struct MidHandshake<S>(Option<MidHandshakeTlsStream<AllowStd<S>>>);

impl<S: AsyncRead + AsyncWrite + Unpin> Future for MidHandshake<S> {
    type Output = Result<TlsStream<S>, io::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut_self = self.get_mut();
        let mut s = mut_self.0.take().expect("future polled after completion");

        s.get_mut().context = cx as *mut _ as *mut ();
        match s.handshake() {
            Ok(mut s) => {
                s.get_mut().context = null_mut();
                Poll::Ready(Ok(TlsStream(s)))
            }
            Err(HandshakeError::Interrupted(mut s)) => {
                s.get_mut().context = null_mut();
                mut_self.0 = Some(s);
                Poll::Pending
            }
            Err(HandshakeError::Failure(e)) => Poll::Ready(Err(e)),
        }
    }
}

enum StartedHandshake<S> {
    Done(TlsStream<S>),
    Mid(MidHandshakeTlsStream<AllowStd<S>>),
}

struct StartedHandshakeFuture<F, S>(Option<StartedHandshakeFutureInner<F, S>>);
struct StartedHandshakeFutureInner<F, S> {
    f: F,
    stream: S,
}

async fn handshake<F, S>(f: F, stream: S) -> Result<TlsStream<S>, io::Error>
where
    F: FnOnce(
            AllowStd<S>,
        ) -> Result<InnerTlsStream<S>, HandshakeError<AllowStd<S>>>
        + Unpin,
    S: AsyncRead + AsyncWrite + Unpin,
{
    let start = StartedHandshakeFuture(Some(StartedHandshakeFutureInner { f, stream }));

    match start.await {
        Err(e) => Err(e),
        Ok(StartedHandshake::Done(s)) => Ok(s),
        Ok(StartedHandshake::Mid(s)) => MidHandshake(Some(s)).await,
    }
}


impl<F, S> Future for StartedHandshakeFuture<F, S>
where
    F: FnOnce(
            AllowStd<S>,
        ) -> Result<InnerTlsStream<S>, HandshakeError<AllowStd<S>>>
        + Unpin,
    S: Unpin,
    AllowStd<S>: Read + Write,
{
    type Output = Result<StartedHandshake<S>, io::Error>;

    fn poll(
        mut self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
    ) -> Poll<Result<StartedHandshake<S>, io::Error>> {
        let inner = self.0.take().expect("future polled after completion");
        let stream = AllowStd {
            inner: inner.stream,
            context: ctx as *mut _ as *mut (),
        };

        match (inner.f)(stream) {
            Ok(mut s) => {
                s.get_mut().context = null_mut();
                Poll::Ready(Ok(StartedHandshake::Done(TlsStream(s))))
            }
            Err(HandshakeError::Interrupted(mut s)) => {
                s.get_mut().context = null_mut();
                Poll::Ready(Ok(StartedHandshake::Mid(s)))
            }
            Err(HandshakeError::Failure(e)) => Poll::Ready(Err(e)),
        }
    }
}