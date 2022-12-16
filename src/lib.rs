//! Async TLS streams
//!
//! This library is an implementation of TLS streams for Windows using SCannel.

mod allow_std;
mod guard;
mod tls_stream;
mod builder;

use crate::allow_std::AllowStd;

type InnerTlsStream<S> = schannel::tls_stream::TlsStream<AllowStd<S>>;
type InnerBuilder = schannel::tls_stream::Builder;

pub use builder::Builder;
pub use tls_stream::TlsStream;