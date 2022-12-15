use std::{task::{Context, Poll}, io::{self, Read, Write}, pin::Pin, os::windows::prelude::{AsRawSocket, RawSocket}};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::{InnerTlsStream, allow_std::AllowStd, guard::Guard};

/// A wrapper around the SChannel TlsStream which implements the TLS or SSL
/// protocol.
///
/// A `TlsStream<S>` represents a handshake that has been completed successfully
/// and both the server and the client are ready for receiving and sending
/// data. Bytes read from a `TlsStream` are decrypted from `S` and bytes written
/// to a `TlsStream` are encrypted when passing through to `S`.
#[derive(Debug)]
pub struct TlsStream<S>(pub(crate) InnerTlsStream<S>);

impl<S> TlsStream<S> {
    fn with_context<F, R>(&mut self, ctx: &mut Context<'_>, f: F) -> Poll<io::Result<R>>
    where
        F: FnOnce(&mut InnerTlsStream<S>) -> io::Result<R>,
        AllowStd<S>: Read + Write,
    {
        self.0.get_mut().context = ctx as *mut _ as *mut ();
        let g = Guard(self);
        match f(&mut (g.0).0) {
            Ok(v) => Poll::Ready(Ok(v)),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    /// Returns a shared reference to the inner stream.
    pub fn get_ref(&self) -> &InnerTlsStream<S> {
        &self.0
    }

    /// Returns a mutable reference to the inner stream.
    pub fn get_mut(&mut self) -> &mut InnerTlsStream<S> {
        &mut self.0
    }
}

impl<S> AsyncRead for TlsStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.with_context(ctx, |s| {
            let n = s.read(buf.initialize_unfilled())?;
            buf.advance(n);
            Ok(())
        })
    }
}

impl<S> AsyncWrite for TlsStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        ctx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.with_context(ctx, |s| s.write(buf))
    }

    fn poll_flush(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.with_context(ctx, |s| s.flush())
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.with_context(ctx, |s| s.shutdown())
    }
}

impl<S> AsRawSocket for TlsStream<S>
where
    S: AsRawSocket,
{
    fn as_raw_socket(&self) -> RawSocket {
        self.get_ref().get_ref().get_ref().as_raw_socket()
    }
}