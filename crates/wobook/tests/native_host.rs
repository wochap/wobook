//! End-to-end: real `wobookd` driven through `wobook native-host` framing.

use std::{
    io::{BufRead, BufReader, Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tempfile::TempDir;

fn wobook_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_wobook"))
}

/// `wobookd` lives in another package; build it next to `wobook` once.
fn wobookd_bin() -> PathBuf {
    static BIN: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    BIN.get_or_init(|| {
        let path = wobook_bin().with_file_name("wobookd");
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let status = Command::new(cargo)
            .args(["build", "-q", "-p", "wobookd"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .expect("cargo build wobookd");
        assert!(status.success());
        path
    })
    .clone()
}

struct Daemon {
    root: TempDir,
    socket: PathBuf,
    child: Option<Child>,
}

impl Daemon {
    fn new(post_add: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let hooks = root.path().join("hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        let hook = hooks.join("post-add.origin");
        std::fs::write(&hook, post_add).unwrap();
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        let socket = root.path().join("run/wobookd.sock");
        let child = Command::new(wobookd_bin())
            .arg("--data-dir")
            .arg(root.path().join("data"))
            .arg("--socket")
            .arg(&socket)
            .arg("--hooks-dir")
            .arg(&hooks)
            .env("WOBOOK_SYNC_PORT", "0")
            .env("WOBOOK_DISCOVERY", "off")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while UnixStream::connect(&socket).is_err() {
            assert!(Instant::now() < deadline, "wobookd did not start");
            std::thread::sleep(Duration::from_millis(50));
        }
        Self {
            root,
            socket,
            child: Some(child),
        }
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            if let Ok(mut s) = UnixStream::connect(&self.socket) {
                let _ = writeln!(s, "{}", json!({"type":"shutdown"}));
                let _ = BufReader::new(s).read_line(&mut String::new());
            }
            let deadline = Instant::now() + Duration::from_secs(10);
            while child.try_wait().unwrap().is_none() {
                if Instant::now() > deadline {
                    let _ = child.kill();
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = child.wait();
            let _ = std::fs::remove_file(&self.socket);
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Host {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: ChildStdout,
}

impl Host {
    fn spawn(socket: &Path) -> Self {
        let mut child = Command::new(wobook_bin())
            .arg("--socket")
            .arg(socket)
            .arg("native-host")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        Self {
            stdin: child.stdin.take(),
            stdout: child.stdout.take().unwrap(),
            child,
        }
    }

    fn send_raw(&mut self, body: &[u8]) {
        let stdin = self.stdin.as_mut().unwrap();
        stdin.write_all(&(body.len() as u32).to_le_bytes()).unwrap();
        stdin.write_all(body).unwrap();
        stdin.flush().unwrap();
    }

    fn recv(&mut self) -> Value {
        let mut len = [0u8; 4];
        self.stdout.read_exact(&mut len).unwrap();
        let mut body = vec![0u8; u32::from_le_bytes(len) as usize];
        self.stdout.read_exact(&mut body).unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    fn call(&mut self, request: Value) -> Value {
        self.send_raw(request.to_string().as_bytes());
        self.recv()
    }

    fn close(mut self) -> std::process::ExitStatus {
        drop(self.stdin.take());
        self.child.wait().unwrap()
    }
}

#[test]
fn proxies_frames_to_the_daemon() {
    let mut d = Daemon::new("");
    let log = d.root.path().join("origin.txt");
    std::fs::write(
        d.root.path().join("hooks/post-add.origin"),
        format!(
            "#!/bin/sh\nprintf '%s' \"$WOBOOK_ORIGIN\" > '{}'\n",
            log.display()
        ),
    )
    .unwrap();
    let mut host = Host::spawn(&d.socket);

    let pong = host.call(json!({"type":"ping"}));
    assert_eq!(pong["ok"], true, "{pong}");
    assert!(pong["result"]["version"].is_string(), "{pong}");

    let added = host.call(json!({
        "type":"add","url":"https://ui.shadcn.com/docs","title":"shadcn/ui",
        "tags":["react","ui library"],"fetch":false,"merge":true
    }));
    assert_eq!(added["ok"], true, "{added}");
    let deadline = Instant::now() + Duration::from_secs(15);
    let origin = loop {
        if let Ok(t) = std::fs::read_to_string(&log)
            && !t.is_empty()
        {
            break t;
        }
        assert!(Instant::now() < deadline, "post-add hook did not run");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(origin, "extension");

    let got = host.call(json!({"type":"get","url":"https://ui.shadcn.com/docs"}));
    assert_eq!(got["result"]["title"], "shadcn/ui", "{got}");
    let missing = host.call(json!({"type":"get","url":"https://nope.example/"}));
    assert_eq!(missing["error"]["code"], "not_found");

    let hits = host.call(json!({"type":"search","query":"shcn ui","limit":50}));
    assert_eq!(
        hits["result"][0]["bookmark"]["url"], "https://ui.shadcn.com/docs",
        "{hits}"
    );

    // Oversized frame: error, payload skipped, host keeps serving.
    let stdin = host.stdin.as_mut().unwrap();
    let big = 5 * 1024 * 1024u32;
    stdin.write_all(&big.to_le_bytes()).unwrap();
    stdin.write_all(&vec![b' '; big as usize]).unwrap();
    assert_eq!(host.recv()["error"]["code"], "invalid_request");
    assert_eq!(host.call(json!({"type":"ping"}))["ok"], true);

    host.send_raw(b"{not json");
    assert_eq!(host.recv()["error"]["code"], "invalid_request");
    assert_eq!(
        host.call(json!({"type":"nope"}))["error"]["code"],
        "invalid_request"
    );

    d.stop();
    let down = host.call(json!({"type":"get","url":"https://ui.shadcn.com/docs"}));
    assert_eq!(down["error"]["code"], "daemon_unavailable", "{down}");
    assert!(
        down["error"]["message"]
            .as_str()
            .unwrap()
            .contains("wobookd is not running")
    );

    assert_eq!(host.close().code(), Some(0));
}

fn manifest(args: &[&str]) -> std::process::Output {
    Command::new(wobook_bin())
        .args(["native-host", "--print-manifest"])
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn prints_manifests() {
    let out = manifest(&["firefox", "--binary", "/usr/bin/wobook-native-host"]);
    assert!(out.status.success());
    let m: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(m["name"], "dev.wochap.wobook");
    assert_eq!(m["type"], "stdio");
    assert_eq!(m["path"], "/usr/bin/wobook-native-host");
    assert_eq!(m["allowed_extensions"], json!(["wobook@wochap.dev"]));

    let out = manifest(&["chrome"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--extension-id"));

    let id = "abcdefghijklmnopabcdefghijklmnop";
    for browser in ["chrome", "brave"] {
        let out = manifest(&[browser, "--extension-id", id]);
        assert!(out.status.success());
        let m: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(
            m["allowed_origins"],
            json!([format!("chrome-extension://{id}/")])
        );
        let path = m["path"].as_str().unwrap();
        assert!(
            path.starts_with('/') && path.ends_with("/wobook-native-host"),
            "{path}"
        );
    }

    let help = Command::new(wobook_bin()).arg("--help").output().unwrap();
    assert!(String::from_utf8_lossy(&help.stdout).contains("native-host"));
}
