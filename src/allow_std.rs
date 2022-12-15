use std::io::{Write, Read, self};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// An intermediate wrapper for the inner stream `S`.
#[derive(Debug)]
pub struct AllowStd<S> {
    pub(crate) inner: S,
    pub(crate) context: *mut (),
}

impl<S> AllowStd<S> {
    /// Returns a shared reference to the inner stream.
    pub fn get_ref(&self) -> &S {
        &self.inner
    }

    /// Returns a mutable reference to the inner stream.
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.inner
    }
}

// *mut () context is neither Send nor Sync
unsafe impl<S: Send> Send for AllowStd<S> {}
unsafe impl<S: Sync> Sync for AllowStd<S> {}

impl<S> AllowStd<S>
where
    S: Unpin,
{
    fn with_context<F, R>(&mut self, f: F) -> io::Result<R>
    where
        F: FnOnce(&mut Context<'_>, Pin<&mut S>) -> Poll<io::Result<R>>,
    {
        unsafe {
            assert!(!self.context.is_null());
            let waker = &mut *(self.context as *mut _);
            match f(waker, Pin::new(&mut self.inner)) {
                Poll::Ready(r) => r,
                Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
            }
        }
    }
}

impl<S> Read for AllowStd<S>
where
    S: AsyncRead + Unpin,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut buf = ReadBuf::new(buf);
        self.with_context(|ctx, stream| stream.poll_read(ctx, &mut buf))?;
        Ok(buf.filled().len())
    }
}

impl<S> Write for AllowStd<S>
where
    S: AsyncWrite + Unpin,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.with_context(|ctx, stream| stream.poll_write(ctx, buf))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.with_context(|ctx, stream| stream.poll_flush(ctx))
    }
}