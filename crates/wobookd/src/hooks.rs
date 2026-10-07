//! Hook runner (D11).

use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use serde::Serialize;
use serde_json::{Value, json};
use tokio::{io::AsyncWriteExt, process::Command, sync::mpsc};

pub const EVENTS: &[&str] = &[
    "pre-add",
    "post-add",
    "post-update",
    "post-delete",
    "post-sync",
];
pub const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Serialize)]
pub struct Outcome {
    pub hook: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout: String,
    pub stderr: String,
}

impl Outcome {
    pub fn success(&self) -> bool {
        !self.timed_out && self.exit_code == Some(0)
    }
}

/// Executable files named `<event>` or `<event>.*`, in lexical order.
pub fn discover(dir: &Path, event: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let prefix = format!("{event}.");
    let mut hooks: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (name == event || name.starts_with(&prefix))
                && e.metadata()
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
        .map(|e| e.path())
        .collect();
    hooks.sort();
    hooks
}

/// All hooks across every event.
pub fn list(dir: &Path) -> Vec<(String, PathBuf)> {
    EVENTS
        .iter()
        .flat_map(|event| {
            discover(dir, event)
                .into_iter()
                .map(|p| (event.to_string(), p))
        })
        .collect()
}

pub struct Context<'a> {
    pub event: &'a str,
    pub origin: &'a str,
    pub url: &'a str,
    pub data_dir: &'a Path,
    /// Peer device name for remote-origin and `post-sync` hooks.
    pub peer: Option<&'a str>,
}

pub fn payload(event: &str, origin: &str, bookmark: &Value, previous: Option<&Value>) -> Value {
    json!({
        "event": event,
        "origin": origin,
        "bookmark": bookmark,
        "previous": previous,
    })
}

pub async fn run_one(path: &Path, ctx: &Context<'_>, payload: &Value) -> Outcome {
    let hook = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let fail = |message: String| Outcome {
        hook: hook.clone(),
        exit_code: None,
        timed_out: false,
        stdout: String::new(),
        stderr: message,
    };
    let mut command = Command::new(path);
    if let Some(peer) = ctx.peer {
        command.env("WOBOOK_PEER", peer);
    }
    let mut child = match command
        .env("WOBOOK_EVENT", ctx.event)
        .env("WOBOOK_ORIGIN", ctx.origin)
        .env("WOBOOK_URL", ctx.url)
        .env("WOBOOK_DATA_DIR", ctx.data_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(e) => return fail(format!("spawn failed: {e}")),
    };
    if let Some(mut stdin) = child.stdin.take() {
        let mut line = payload.to_string();
        line.push('\n');
        // A hook that ignores stdin may close it early; that is fine.
        let _ = stdin.write_all(line.as_bytes()).await;
    }
    match tokio::time::timeout(TIMEOUT, child.wait_with_output()).await {
        Ok(Ok(output)) => Outcome {
            hook,
            exit_code: output.status.code(),
            timed_out: false,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        },
        Ok(Err(e)) => fail(format!("wait failed: {e}")),
        Err(_) => Outcome {
            hook,
            exit_code: None,
            timed_out: true,
            stdout: String::new(),
            stderr: "hook timed out".into(),
        },
    }
}

pub async fn run_all(dir: &Path, ctx: &Context<'_>, payload: &Value) -> Vec<Outcome> {
    let mut out = Vec::new();
    for hook in discover(dir, ctx.event) {
        out.push(run_one(&hook, ctx, payload).await);
    }
    out
}

pub fn log_outcome(event: &str, o: &Outcome) {
    eprintln!(
        "wobookd: hook {event}/{} exit={:?} timed_out={}",
        o.hook, o.exit_code, o.timed_out
    );
    for (name, text) in [("stdout", &o.stdout), ("stderr", &o.stderr)] {
        let text = text.trim();
        if !text.is_empty() {
            eprintln!("wobookd: hook {event}/{} {name}: {text}", o.hook);
        }
    }
}

pub struct PostJob {
    pub event: &'static str,
    pub origin: String,
    pub url: String,
    pub payload: Value,
    pub peer: Option<String>,
}

/// Sequential background runner for `post-*` hooks.
pub fn spawn_post_runner(dir: PathBuf, data_dir: PathBuf) -> mpsc::UnboundedSender<PostJob> {
    let (tx, mut rx) = mpsc::unbounded_channel::<PostJob>();
    tokio::spawn(async move {
        while let Some(job) = rx.recv().await {
            let ctx = Context {
                event: job.event,
                origin: &job.origin,
                url: &job.url,
                data_dir: &data_dir,
                peer: job.peer.as_deref(),
            };
            for outcome in run_all(&dir, &ctx, &job.payload).await {
                log_outcome(job.event, &outcome);
            }
        }
    });
    tx
}
