//! Native messaging host (`dev.wochap.wobook`): length-prefixed JSON frames on
//! stdio, each proxied to the daemon socket as one request.

use std::{
    io::{BufRead, BufReader, ErrorKind, Read, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::ExitCode,
};

use serde_json::{Value, json};
use wobook_core::protocol::{ErrorCode, MAX_REQUEST, Request, Response};

pub const HOST_NAME: &str = "dev.wochap.wobook";
pub const GECKO_ID: &str = "wobook@wochap.dev";
/// Browsers kill hosts whose messages exceed 1 MiB.
const MAX_RESPONSE: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Browser {
    Firefox,
    Chrome,
    Brave,
}

/// Prints the host manifest; `Err` carries a usage message (exit 2).
pub fn print_manifest(
    browser: Browser,
    extension_id: Option<String>,
    binary: Option<PathBuf>,
) -> std::result::Result<(), String> {
    let path = match binary {
        Some(p) => std::path::absolute(&p).map_err(|e| e.to_string())?,
        None => std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("wobook-native-host"),
    };
    let mut manifest = json!({
        "name": HOST_NAME,
        "description": "wobook bookmarks",
        "path": path.to_string_lossy(),
        "type": "stdio",
    });
    match browser {
        Browser::Firefox => {
            let id = extension_id.unwrap_or_else(|| GECKO_ID.into());
            manifest["allowed_extensions"] = json!([id]);
        }
        Browser::Chrome | Browser::Brave => {
            let id = extension_id.ok_or("--extension-id is required for chrome and brave")?;
            manifest["allowed_origins"] = json!([format!("chrome-extension://{id}/")]);
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&manifest).unwrap_or_default()
    );
    Ok(())
}

pub fn run(socket: &Path) -> ExitCode {
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    loop {
        let mut len = [0u8; 4];
        match stdin.read_exact(&mut len) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => return ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("wobook native-host: stdin: {e}");
                return ExitCode::SUCCESS;
            }
        }
        let len = u32::from_le_bytes(len) as usize;
        let response = if len > MAX_REQUEST {
            // Skip the payload so the next frame stays aligned.
            match std::io::copy(&mut (&mut stdin).take(len as u64), &mut std::io::sink()) {
                Ok(n) if n as usize == len => {}
                _ => return ExitCode::SUCCESS,
            }
            Response::err_json(ErrorCode::InvalidRequest, "message exceeds 4 MiB")
        } else {
            let mut body = vec![0u8; len];
            if stdin.read_exact(&mut body).is_err() {
                return ExitCode::SUCCESS;
            }
            handle(socket, &body)
        };
        if write_frame(&mut stdout, &response).is_err() {
            return ExitCode::from(70);
        }
    }
}

fn write_frame(out: &mut impl Write, body: &str) -> std::io::Result<()> {
    let too_large;
    let body = if body.len() > MAX_RESPONSE {
        too_large = Response::err_json(
            ErrorCode::ResponseTooLarge,
            "response exceeds 1 MiB; pass a smaller limit",
        );
        &too_large
    } else {
        body
    };
    out.write_all(&(body.len() as u32).to_le_bytes())?;
    out.write_all(body.as_bytes())?;
    out.flush()
}

fn handle(socket: &Path, body: &[u8]) -> String {
    let mut value: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(e) => return Response::err_json(ErrorCode::InvalidRequest, e.to_string()),
    };
    if let Some(obj) = value.as_object_mut()
        && matches!(obj.get("type").and_then(Value::as_str), Some("add" | "update"))
        && !obj.contains_key("origin")
    {
        obj.insert("origin".into(), json!("extension"));
    }
    if let Err(e) = serde_json::from_value::<Request>(value.clone()) {
        return Response::err_json(ErrorCode::InvalidRequest, e.to_string());
    }
    let Ok(mut stream) = UnixStream::connect(socket) else {
        return Response::err_json(
            ErrorCode::DaemonUnavailable,
            "wobookd is not running (start it: systemctl --user start wobookd)",
        );
    };
    if writeln!(stream, "{value}").is_err() {
        return Response::err_json(ErrorCode::DaemonUnavailable, "wobookd closed the socket");
    }
    let mut reply = String::new();
    match BufReader::new(stream).read_line(&mut reply) {
        Ok(n) if n > 0 => reply.trim_end().to_string(),
        _ => Response::err_json(
            ErrorCode::DaemonUnavailable,
            "wobookd closed the connection",
        ),
    }
}
