//! End-to-end p2p sync (D14): several `wobookd` on 127.0.0.1 with distinct
//! data directories, fixed ports and discovery off, driven by `wobook`.

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::UdpSocket,
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    sync::OnceLock,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tempfile::TempDir;

fn wobookd_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_wobookd"))
}

fn wobook_bin() -> PathBuf {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let path = wobookd_bin().with_file_name("wobook");
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let status = Command::new(cargo)
            .args(["build", "-q", "-p", "wobook"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .expect("cargo build wobook");
        assert!(status.success());
        path
    })
    .clone()
}

fn free_udp_port() -> u16 {
    UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn wait_until(what: &str, timeout: Duration, mut f: impl FnMut() -> bool) {
    let deadline = Instant::now() + timeout;
    while !f() {
        assert!(Instant::now() < deadline, "timed out: {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

struct Node {
    root: TempDir,
    name: String,
    data: PathBuf,
    socket: PathBuf,
    hooks: PathBuf,
    port: u16,
    child: Option<Child>,
}

impl Node {
    fn new(name: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data");
        let socket = root.path().join("run/wobookd.sock");
        let hooks = root.path().join("hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        let mut node = Self {
            root,
            name: name.into(),
            data,
            socket,
            hooks,
            port: free_udp_port(),
            child: None,
        };
        node.start();
        node.req(&json!({"type":"device.name","name":name}));
        node
    }

    fn start(&mut self) {
        let mut child = Command::new(wobookd_bin())
            .arg("--data-dir")
            .arg(&self.data)
            .arg("--socket")
            .arg(&self.socket)
            .arg("--hooks-dir")
            .arg(&self.hooks)
            .env_remove("WOBOOK_DATA_DIR")
            .env_remove("WOBOOK_SOCKET")
            .env_remove("WOBOOK_HOOKS_DIR")
            .env_remove("WOBOOK_DEVICE_NAME")
            .env("WOBOOK_SYNC_PORT", self.port.to_string())
            .env("WOBOOK_DISCOVERY", "off")
            .env("WOBOOK_SYNC_LOOPBACK", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        while self.raw(&json!({"type":"ping"})).is_none() {
            if let Ok(Some(status)) = child.try_wait() {
                let mut err = String::new();
                child
                    .stderr
                    .take()
                    .unwrap()
                    .read_to_string(&mut err)
                    .unwrap();
                panic!("wobookd {} exited {status}: {err}", self.name);
            }
            assert!(Instant::now() < deadline, "wobookd did not start");
            std::thread::sleep(Duration::from_millis(50));
        }
        let stderr = child.stderr.take().unwrap();
        let name = self.name.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                eprintln!("[{name}] {line}");
            }
        });
        self.child = Some(child);
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = self.raw(&json!({"type":"shutdown"}));
            let deadline = Instant::now() + Duration::from_secs(10);
            while child.try_wait().unwrap().is_none() {
                if Instant::now() > deadline {
                    let _ = child.kill();
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = child.wait();
        }
    }

    fn restart(&mut self) {
        self.stop();
        self.start();
    }

    fn raw(&self, value: &Value) -> Option<Value> {
        let mut stream = UnixStream::connect(&self.socket).ok()?;
        writeln!(stream, "{value}").ok()?;
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).ok()?;
        serde_json::from_str(&line).ok()
    }

    fn req(&self, value: &Value) -> Value {
        let r = self.raw(value).expect("daemon reachable");
        assert_eq!(r["ok"], true, "{} {value}: {r}", self.name);
        r["result"].clone()
    }

    fn err_code(&self, value: &Value) -> String {
        let r = self.raw(value).expect("daemon reachable");
        assert_eq!(r["ok"], false, "{value} unexpectedly succeeded: {r}");
        r["error"]["code"].as_str().unwrap().to_string()
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::new(wobook_bin());
        cmd.arg("--socket")
            .arg(&self.socket)
            .env_remove("WOBOOK_SOCKET");
        cmd
    }

    fn cli(&self, args: &[&str]) -> Output {
        self.cmd().args(args).output().unwrap()
    }

    fn ok(&self, args: &[&str]) -> String {
        let out = self.cli(args);
        assert!(
            out.status.success(),
            "{}: wobook {args:?} failed: {}",
            self.name,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    fn add(&self, url: &str, tags: &[&str]) {
        self.req(&json!({"type":"add","url":url,"tags":tags,"fetch":false,"title":url}));
    }

    fn list(&self) -> Vec<Value> {
        self.req(&json!({"type":"list"}))
            .as_array()
            .unwrap()
            .clone()
    }

    fn urls(&self) -> Vec<String> {
        let mut urls: Vec<String> = self
            .list()
            .iter()
            .map(|b| b["url"].as_str().unwrap().to_string())
            .collect();
        urls.sort();
        urls
    }

    fn id(&self) -> String {
        self.req(&json!({"type":"status"}))["device"]["id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn heads(&self) -> String {
        self.req(&json!({"type":"status"}))["heads"]
            .as_str()
            .unwrap()
            .to_string()
    }

    fn peer<'a>(&self, status: &'a Value, other: &Node) -> &'a Value {
        let id = other.id();
        status["peers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id.as_str())
            .unwrap_or_else(|| panic!("{} does not list {}: {status}", self.name, other.name))
    }

    fn sync_peer(&self, other: &Node) -> Value {
        let status = self.req(&json!({"type":"sync.status"}));
        self.peer(&status, other).clone()
    }

    fn device(&self, other: &Node) -> Option<Value> {
        let id = other.id();
        self.req(&json!({"type":"devices.list"}))
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["id"] == id.as_str())
            .cloned()
    }

    fn connected(&self, other: &Node) -> bool {
        self.sync_peer(other)["connected"] == true
    }

    fn hook(&self, name: &str, body: &str) {
        let path = self.hooks.join(name);
        std::fs::write(&path, body).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn file(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Starts `wobook pair --json --yes` on the offerer and returns the child and
/// the payload line.
fn offer(offerer: &Node) -> (Child, String) {
    let mut child = offerer
        .cmd()
        .args(["pair", "--json", "--yes"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert!(line.starts_with('{'), "payload: {line}");
    (child, line)
}

fn join(joiner: &Node, payload: &str) -> Output {
    let mut child = joiner
        .cmd()
        .args(["pair", "--join", "-", "--yes"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(payload.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn finish(child: Child) -> Output {
    child.wait_with_output().unwrap()
}

/// `wobook pair --json --yes | wobook pair --join - --yes`, then waits for the
/// joiner to hold the offerer's document and a live connection.
fn pair(offerer: &Node, joiner: &Node) {
    let (child, payload) = offer(offerer);
    let joined = join(joiner, &payload);
    let offered = finish(child);
    assert!(
        joined.status.success(),
        "join failed: {}",
        String::from_utf8_lossy(&joined.stderr)
    );
    assert!(
        offered.status.success(),
        "offer failed: {}",
        String::from_utf8_lossy(&offered.stderr)
    );
    wait_until("paired peers connect", Duration::from_secs(30), || {
        offerer.connected(joiner) && joiner.connected(offerer)
    });
    wait_until("joiner synced", Duration::from_secs(30), || {
        joiner.heads() == offerer.heads()
    });
}

fn converge(a: &Node, b: &Node) {
    wait_until(
        &format!("{} and {} converge", a.name, b.name),
        Duration::from_secs(15),
        || a.heads() == b.heads() && a.urls() == b.urls(),
    );
}

#[test]
fn pair_sync_status_and_names() {
    let a = Node::new("alpha");
    let b = Node::new("beta");
    a.add("https://a.example/one", &["x"]);
    pair(&a, &b);
    assert_eq!(b.urls(), vec!["https://a.example/one".to_string()]);

    // Add on A appears on B within 5 s.
    a.add("https://a.example/two", &[]);
    wait_until("add propagates", Duration::from_secs(5), || {
        b.urls().len() == 2
    });

    // devices.list shape.
    let dev = b.device(&a).expect("B lists A");
    assert_eq!(dev["name"], "alpha");
    assert_eq!(dev["platform"], "linux");
    assert_eq!(dev["revoked"], false);
    assert!(dev["paired_at_ms"].as_i64().unwrap() > 0);
    assert!(!dev["endpoints"].as_array().unwrap().is_empty());
    let listed = a.ok(&["devices", "list", "--json"]);
    assert!(listed.contains("beta"));

    // sync.status shape and status.peers.
    let st = a.req(&json!({"type":"sync.status"}));
    assert_eq!(st["device"]["name"], "alpha");
    assert_eq!(st["device"]["port"], a.port);
    let p = a.peer(&st, &b);
    assert_eq!(p["connected"], true);
    assert_eq!(p["reachability"], "lan");
    wait_until("heads_equal", Duration::from_secs(10), || {
        a.sync_peer(&b)["heads_equal"] == true
    });
    assert!(a.sync_peer(&b)["last_sync_ms"].as_i64().is_some());
    let status = a.req(&json!({"type":"status"}));
    assert_eq!(status["peers"], json!({"trusted": 1, "connected": 1}));
    assert_eq!(status["device"]["id"].as_str().unwrap().len(), 64);
    assert!(status.get("recovery").is_none());
    let text = a.ok(&["sync", "status"]);
    assert!(
        text.contains("beta") && text.contains("lan") && text.contains("ago"),
        "{text}"
    );

    // device name get/set and validation; rename propagates.
    assert_eq!(a.ok(&["device", "name"]).trim(), "alpha");
    a.ok(&["device", "name", "laptop"]);
    assert_eq!(a.req(&json!({"type":"device.name"}))["name"], "laptop");
    assert_eq!(a.req(&json!({"type":"status"}))["device"]["name"], "laptop");
    assert_eq!(
        a.err_code(&json!({"type":"device.name","name":""})),
        "invalid_request"
    );
    wait_until("rename propagates", Duration::from_secs(10), || {
        b.device(&a).unwrap()["name"] == "laptop"
    });

    // Identity key file is private and stable.
    let mode = std::fs::metadata(a.data.join("identity.key"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn offline_edits_reconnect_and_endpoints() {
    let a = Node::new("alpha");
    let mut b = Node::new("beta");
    a.add("https://shared.example/", &["base"]);
    pair(&a, &b);

    // Concurrent offline edits on the same and different bookmarks.
    b.stop();
    a.req(&json!({"type":"update","url":"https://shared.example/","add_tags":["from-a"]}));
    a.add("https://a-only.example/", &[]);
    let saved_a_port = a.port;
    b.start();
    // B is up but A has not reconnected yet when B dials; edit B immediately.
    b.req(&json!({"type":"update","url":"https://shared.example/","add_tags":["from-b"]}));
    b.add("https://b-only.example/", &[]);
    b.ok(&["sync", "now"]);
    converge(&a, &b);
    let shared = a
        .list()
        .into_iter()
        .find(|x| x["url"] == "https://shared.example/")
        .unwrap();
    let tags: Vec<&str> = shared["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap())
        .collect();
    assert_eq!(tags, vec!["base", "from-a", "from-b"]);
    assert_eq!(a.list(), b.list());

    // Endpoint memory survives restart, A reconnects to B on its own.
    let before = a.device(&b).unwrap()["endpoints"].clone();
    assert!(
        before
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["last_success_ms"].is_i64())
    );
    b.restart();
    wait_until(
        "A reconnects after B restarts",
        Duration::from_secs(30),
        || a.connected(&b),
    );
    assert_eq!(a.port, saved_a_port);
    let after = a.device(&b).unwrap()["endpoints"].clone();
    assert!(!after.as_array().unwrap().is_empty());

    // Manual endpoint: wipe what A knows by revoking nothing; add one and use it.
    let manual = format!("127.0.0.1:{}", b.port);
    a.ok(&["devices", "add-endpoint", "beta", &manual]);
    let eps = a.device(&b).unwrap()["endpoints"].clone();
    assert!(
        eps.as_array()
            .unwrap()
            .iter()
            .any(|e| e["address"] == manual.as_str())
    );
    assert_eq!(
        a.err_code(&json!({"type":"devices.add_endpoint","id":b.id(),"address":"not-an-address"})),
        "invalid_request"
    );
    assert_eq!(
        a.err_code(&json!({"type":"devices.rename","id":"0".repeat(64),"name":"x"})),
        "unknown_device"
    );
}

#[test]
fn revoke_and_pairing_failures() {
    let a = Node::new("alpha");
    let b = Node::new("beta");
    let c = Node::new("gamma");
    pair(&a, &b);

    // Revoke B on A: connection closes, B cannot reconnect, the row stays.
    a.ok(&["devices", "revoke", "beta", "--yes"]);
    wait_until("B disconnected", Duration::from_secs(5), || {
        !b.connected(&a)
    });
    b.ok(&["sync", "now"]);
    wait_until("B sees device_revoked", Duration::from_secs(15), || {
        b.sync_peer(&a)["last_error"] == "device_revoked"
    });
    assert!(!a.connected(&b));
    let row = a.device(&b).unwrap();
    assert_eq!(row["revoked"], true);
    assert!(row["endpoints"].as_array().unwrap().is_empty());
    assert_eq!(
        a.err_code(&json!({"type":"devices.rename","id":b.id(),"name":"x"})),
        "device_revoked"
    );
    assert_eq!(
        a.err_code(&json!({"type":"pair.confirm","session":"nope"})),
        "pair_pending_missing"
    );

    // Wrong secret: bad_proof, window stays open; third failure locks the key.
    let (offerer, payload) = offer(&a);
    let mut wrong: Value = serde_json::from_str(&payload).unwrap();
    wrong["s"] = json!("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    let wrong = wrong.to_string();
    for _ in 0..3 {
        let out = join(&c, &wrong);
        assert!(!out.status.success());
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(err.contains("bad_proof"), "{err}");
    }
    let out = join(&c, &wrong);
    assert!(String::from_utf8_lossy(&out.stderr).contains("rate_limited"));
    // The window is still open for a different joiner with the right secret.
    let d = Node::new("delta");
    let out = join(&d, &payload);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(finish(offerer).status.success());
    // Second joiner after success: expired.
    let out = join(&c, &payload);
    assert!(String::from_utf8_lossy(&out.stderr).contains("expired"));

    // Expired payload is rejected before connecting.
    let mut old: Value = serde_json::from_str(&payload).unwrap();
    old["exp"] = json!(old["exp"].as_i64().unwrap() - 600);
    assert_eq!(
        c.err_code(&json!({"type":"pair.join","payload":old})),
        "invalid_request"
    );
    assert_eq!(
        c.err_code(&json!({"type":"pair.join","payload":{"v":1,"name":"x","id":"zz","ep":[],"s":"","exp":0}})),
        "invalid_request"
    );

    // One side rejects: neither stores trust.
    let (offerer, payload) = offer(&a);
    let started = c.req(
        &json!({"type":"pair.join","payload":serde_json::from_str::<Value>(&payload).unwrap()}),
    );
    let session = started["session"].as_str().unwrap().to_string();
    wait_until("joiner confirming", Duration::from_secs(10), || {
        c.req(&json!({"type":"pair.pending"}))
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["session"] == session.as_str() && s["stage"] == "confirming")
    });
    // Fingerprint shape, identical on both sides.
    let fp_c = c
        .req(&json!({"type":"pair.pending"}))
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["session"] == session.as_str())
        .unwrap()["fingerprint"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(fp_c.len(), 35);
    assert_eq!(fp_c.split(' ').count(), 4);
    c.req(&json!({"type":"pair.reject","session":session}));
    let out = finish(offerer);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("rejected"));
    assert!(a.device(&c).is_none());
    assert!(c.device(&a).is_none());
}

#[test]
fn join_with_existing_data_and_mesh() {
    let a = Node::new("alpha");
    let b = Node::new("beta");
    for i in 0..20 {
        a.add(&format!("https://a{i}.example/"), &["from-a"]);
    }
    for i in 0..10 {
        let url = if i < 3 {
            format!("https://a{i}.example/")
        } else {
            format!("https://b{i}.example/")
        };
        b.add(&url, &["from-b"]);
    }
    pair(&a, &b);
    wait_until("27 bookmarks on both", Duration::from_secs(30), || {
        a.list().len() == 27 && b.list().len() == 27
    });
    converge(&a, &b);
    let shared = a
        .list()
        .into_iter()
        .find(|x| x["url"] == "https://a0.example/")
        .unwrap();
    assert_eq!(shared["tags"], json!(["from-a", "from-b"]));
    let names: Vec<String> = std::fs::read_dir(&b.data)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names
            .iter()
            .any(|n| n.starts_with("pre-join-") && n.ends_with(".jsonl")),
        "{names:?}"
    );
    let quarantined: Vec<String> = std::fs::read_dir(b.data.join("quarantine"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        quarantined.iter().any(|n| n.starts_with("pre-join-")),
        "{quarantined:?}"
    );

    // Three-device mesh: C pairs with A, B learns C, C learns B.
    let c = Node::new("gamma");
    pair(&a, &c);
    wait_until("B trusts C", Duration::from_secs(10), || {
        b.device(&c).is_some()
    });
    assert!(c.device(&b).is_some());
    wait_until("C converges", Duration::from_secs(30), || {
        c.list().len() == 27
    });
    wait_until("B and C connect", Duration::from_secs(30), || {
        b.connected(&c)
    });
    c.add("https://c.example/", &[]);
    wait_until("C's add reaches B", Duration::from_secs(10), || {
        b.list().len() == 28
    });

    // A device already in another mesh is refused with `protocol`.
    let x = Node::new("xray");
    let y = Node::new("yankee");
    pair(&x, &y);
    let (offerer, payload) = offer(&a);
    let before = y.urls();
    let out = join(&y, &payload);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("protocol"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = finish(offerer);
    assert_eq!(y.urls(), before);
    assert!(a.device(&y).is_none());
}

#[test]
fn recovery_and_hooks() {
    let a = Node::new("alpha");
    let mut b = Node::new("beta");
    for i in 0..5 {
        a.add(&format!("https://r{i}.example/"), &[]);
    }
    pair(&a, &b);

    // Hooks on A for B's changes.
    let log = a.file("hooks.log");
    let body = format!(
        "#!/bin/sh\necho \"$WOBOOK_EVENT $WOBOOK_ORIGIN $WOBOOK_PEER $(cat)\" >> {}\n",
        log.display()
    );
    a.hook("post-add", &body);
    a.hook("post-sync", &body);
    for i in 0..3 {
        b.add(&format!("https://hook{i}.example/"), &[]);
    }
    wait_until("post-sync ran", Duration::from_secs(15), || {
        std::fs::read_to_string(&log).is_ok_and(|t| t.contains("post-sync"))
    });
    std::thread::sleep(Duration::from_millis(500));
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(text.contains("post-add remote:beta beta"), "{text}");
    let adds = text.lines().filter(|l| l.starts_with("post-add")).count();
    assert_eq!(adds, 3, "{text}");
    let changed: usize = text
        .lines()
        .filter(|l| l.starts_with("post-sync"))
        .map(|l| {
            let json = &l[l.find('{').unwrap()..];
            serde_json::from_str::<Value>(json).unwrap()["changed"]
                .as_u64()
                .unwrap() as usize
        })
        .sum();
    assert_eq!(changed, 3, "{text}");
    // A local add triggers post-add with a local origin and no post-sync.
    let syncs = text.lines().filter(|l| l.starts_with("post-sync")).count();
    a.add("https://local.example/", &[]);
    b.ok(&["sync", "now"]);
    std::thread::sleep(Duration::from_secs(1));
    let text = std::fs::read_to_string(&log).unwrap();
    assert_eq!(
        text.lines().filter(|l| l.starts_with("post-sync")).count(),
        syncs,
        "{text}"
    );
    converge(&a, &b);

    // Document deleted on B: B refills from A.
    let expected = a.urls();
    b.stop();
    let docs = b.data.join("automerge");
    for entry in std::fs::read_dir(&docs).unwrap() {
        std::fs::remove_file(entry.unwrap().path()).unwrap();
    }
    b.start();
    wait_until("B refills after deletion", Duration::from_secs(30), || {
        b.urls() == expected
    });
    wait_until("recovery cleared", Duration::from_secs(10), || {
        b.req(&json!({"type":"status"})).get("recovery").is_none()
    });

    // Truncated document: quarantined, one attempt counted, resynced.
    b.stop();
    for entry in std::fs::read_dir(&docs).unwrap() {
        let path = entry.unwrap().path();
        std::fs::write(&path, b"garbage").unwrap();
    }
    b.start();
    let quarantined = std::fs::read_dir(b.data.join("quarantine"))
        .map(|r| r.count())
        .unwrap_or(0);
    assert!(quarantined >= 1);
    let attempts: Vec<String> = std::fs::read_dir(b.data.join("control"))
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            p.file_name()?
                .to_str()?
                .starts_with("recovery-")
                .then(|| std::fs::read_to_string(&p).unwrap())
        })
        .collect();
    // The counter is cumulative per root (the deletion above counted too).
    assert!(
        attempts
            .iter()
            .any(|a| a.trim().parse::<u32>().is_ok_and(|n| n >= 1)),
        "{attempts:?}"
    );
    wait_until(
        "B resyncs after corruption",
        Duration::from_secs(30),
        || b.urls() == expected,
    );
}
