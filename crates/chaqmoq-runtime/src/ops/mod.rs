//! Native operations ("ops") exposed to JavaScript.
//!
//! Every op name is prefixed with `op_chq_` so that it can never collide with an
//! op that `deno_core` itself installs.

pub mod crypto;
pub mod encoding;
pub mod fs;
pub mod http;
pub mod os;
pub mod url;

use std::io::ErrorKind;

use deno_error::JsErrorBox;

/// Converts an [`std::io::Error`] into a JS error whose *class* matches the
/// standard runtime error names, keeping the offending path in the message.
pub(crate) fn io_error(err: std::io::Error, context: &str) -> JsErrorBox {
    let class = match err.kind() {
        ErrorKind::NotFound => "NotFound",
        ErrorKind::PermissionDenied => "PermissionDenied",
        ErrorKind::AlreadyExists => "AlreadyExists",
        ErrorKind::ConnectionRefused => "ConnectionRefused",
        ErrorKind::ConnectionReset => "ConnectionReset",
        ErrorKind::ConnectionAborted => "ConnectionAborted",
        ErrorKind::NotConnected => "NotConnected",
        ErrorKind::AddrInUse => "AddrInUse",
        ErrorKind::AddrNotAvailable => "AddrNotAvailable",
        ErrorKind::BrokenPipe => "BrokenPipe",
        ErrorKind::InvalidInput | ErrorKind::InvalidData => "TypeError",
        ErrorKind::TimedOut => "TimedOut",
        ErrorKind::WouldBlock => "WouldBlock",
        ErrorKind::Unsupported => "NotSupported",
        _ => "Error",
    };
    JsErrorBox::new(class, format!("{err}: {context}"))
}
