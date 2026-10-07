//! Socket client: one request, one response.

use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::Path,
};

use serde_json::Value;
use wobook_core::protocol::{ErrorCode, Request, Response};

pub enum ClientError {
    Unavailable,
    Remote(ErrorCode, String),
    Internal(String),
}

pub fn call(socket: &Path, request: &Request) -> Result<Value, ClientError> {
    let mut stream = UnixStream::connect(socket).map_err(|_| ClientError::Unavailable)?;
    let mut line =
        serde_json::to_string(request).map_err(|e| ClientError::Internal(e.to_string()))?;
    line.push('\n');
    stream
        .write_all(line.as_bytes())
        .map_err(|e| ClientError::Internal(e.to_string()))?;
    let mut reply = String::new();
    BufReader::new(stream)
        .read_line(&mut reply)
        .map_err(|e| ClientError::Internal(e.to_string()))?;
    if reply.trim().is_empty() {
        return Err(ClientError::Internal("daemon closed the connection".into()));
    }
    let response: Response =
        serde_json::from_str(&reply).map_err(|e| ClientError::Internal(e.to_string()))?;
    match (response.ok, response.error) {
        (true, _) => Ok(response.result.unwrap_or(Value::Null)),
        (false, Some(err)) => Err(ClientError::Remote(err.code, err.message)),
        (false, None) => Err(ClientError::Internal("error without body".into())),
    }
}
