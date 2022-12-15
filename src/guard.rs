use std::{io::{Read, Write}, ptr::null_mut};

use crate::{allow_std::AllowStd, tls_stream::TlsStream};

pub(crate) struct Guard<'a, S>(pub(crate) &'a mut TlsStream<S>)
where
    AllowStd<S>: Read + Write;

impl<S> Drop for Guard<'_, S>
where
    AllowStd<S>: Read + Write,
{
    fn drop(&mut self) {
        (self.0).0.get_mut().context = null_mut();
    }
}