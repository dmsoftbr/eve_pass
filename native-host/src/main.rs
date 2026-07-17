//! EVEPass native-messaging bridge (Fase 5A).
//!
//! Chrome launches this tiny binary with stdio when the extension opens a native
//! port. It owns **no** vault state: it reads the extension's Chrome-framed JSON
//! requests, forwards them over a local Unix socket to the running desktop app
//! (which holds the `Session` + keys), and writes the app's reply back to Chrome.
//!
//! - Chrome framing: 4-byte native-endian length + UTF-8 JSON (both directions).
//! - Socket framing: newline-delimited JSON to `~/.evepass/host.sock`.
//! - The extension origin (Chrome's argv[1], `chrome-extension://<id>/`) is
//!   injected as `_origin` so the app can enforce user-approved pairing.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use serde_json::{json, Value};

fn socket_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    home.join(".evepass").join("host.sock")
}

/// The `chrome-extension://<id>/` origin Chrome passes as the first CLI arg.
fn caller_origin() -> String {
    std::env::args()
        .find(|a| a.starts_with("chrome-extension://"))
        .unwrap_or_default()
}

/// Read one Chrome native-messaging frame from stdin, or `None` at EOF.
fn read_frame(stdin: &mut impl Read) -> Option<Value> {
    let mut len_buf = [0u8; 4];
    if stdin.read_exact(&mut len_buf).is_err() {
        return None; // clean EOF — Chrome closed the port
    }
    let len = u32::from_ne_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    if stdin.read_exact(&mut buf).is_err() {
        return None;
    }
    serde_json::from_slice(&buf).ok()
}

/// Write one Chrome native-messaging frame to stdout.
fn write_frame(stdout: &mut impl Write, value: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec());
    stdout.write_all(&(body.len() as u32).to_ne_bytes())?;
    stdout.write_all(&body)?;
    stdout.flush()
}

/// Forward one request to the app over the socket and return its reply. On any
/// socket failure, synthesize a reply so the extension degrades gracefully.
fn forward(mut req: Value, origin: &str) -> Value {
    if let Value::Object(ref mut map) = req {
        map.insert("_origin".into(), Value::String(origin.to_string()));
    }
    match UnixStream::connect(socket_path()) {
        Ok(stream) => {
            let mut writer = match stream.try_clone() {
                Ok(s) => s,
                Err(_) => return app_down(&req),
            };
            let mut line = serde_json::to_string(&req).unwrap_or_default();
            line.push('\n');
            if writer.write_all(line.as_bytes()).is_err() {
                return app_down(&req);
            }
            let mut reader = BufReader::new(stream);
            let mut resp = String::new();
            if reader.read_line(&mut resp).unwrap_or(0) == 0 {
                return app_down(&req);
            }
            serde_json::from_str(resp.trim()).unwrap_or_else(|_| app_down(&req))
        }
        Err(_) => app_down(&req),
    }
}

/// Reply used when the desktop app isn't running/reachable. `status` reports a
/// locked vault (so the popup shows "locked"); other calls report the error.
fn app_down(req: &Value) -> Value {
    if req["type"].as_str() == Some("status") {
        json!({ "locked": true })
    } else {
        json!({ "error": "EVEPass não está em execução" })
    }
}

fn main() {
    let origin = caller_origin();
    let mut stdin = io::stdin().lock();
    let mut stdout = io::stdout().lock();
    while let Some(req) = read_frame(&mut stdin) {
        let resp = forward(req, &origin);
        if write_frame(&mut stdout, &resp).is_err() {
            break;
        }
    }
}
