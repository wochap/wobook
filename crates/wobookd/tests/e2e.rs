//! End-to-end: real `wobookd` driven by the real `wobook` binary.

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::OnceLock,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tempfile::TempDir;

fn wobookd_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_wobookd"))
}

/// `wobook` lives in another package; build it next to `wobookd` once.
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
        assert!(path.exists(), "missing {}", path.display());
        path
    })
    .clone()
}

struct Daemon {
    _root: TempDir,
    data: PathBuf,
    socket: PathBuf,
    hooks: PathBuf,
    child: Option<Child>,
}

fn write_exec(path: &Path, body: &str) {
    std::fs::write(path, body).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

impl Daemon {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data");
        let socket = root.path().join("run/wobookd.sock");
        let hooks = root.path().join("hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        let mut d = Self {
            _root: root,
            data,
            socket,
            hooks,
            child: None,
        };
        d.start();
        d
    }

    fn spawn_raw(&self) -> Child {
        Command::new(wobookd_bin())
            .arg("--data-dir")
            .arg(&self.data)
            .arg("--socket")
            .arg(&self.socket)
            .arg("--hooks-dir")
            .arg(&self.hooks)
            .env_remove("WOBOOK_DATA_DIR")
            .env_remove("WOBOOK_SOCKET")
            .env_remove("WOBOOK_HOOKS_DIR")
            .env("WOBOOK_SYNC_PORT", "0")
            .env("WOBOOK_DISCOVERY", "off")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    }

    fn start(&mut self) {
        let mut child = self.spawn_raw();
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if self.request(&json!({"type":"ping"})).is_some() {
                break;
            }
            if let Ok(Some(status)) = child.try_wait() {
                let mut err = String::new();
                child
                    .stderr
                    .take()
                    .unwrap()
                    .read_to_string(&mut err)
                    .unwrap();
                panic!("wobookd exited {status}: {err}");
            }
            assert!(Instant::now() < deadline, "wobookd did not start");
            std::thread::sleep(Duration::from_millis(50));
        }
        // Drain stderr so hooks logging never blocks the daemon.
        let stderr = child.stderr.take().unwrap();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                eprintln!("[wobookd] {line}");
            }
        });
        self.child = Some(child);
    }

    fn request(&self, value: &Value) -> Option<Value> {
        let mut stream = UnixStream::connect(&self.socket).ok()?;
        writeln!(stream, "{value}").ok()?;
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).ok()?;
        serde_json::from_str(&line).ok()
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = self.request(&json!({"type":"shutdown"}));
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

    fn kill9(&mut self) {
        if let Some(mut child) = self.child.take() {
            child.kill().unwrap();
            child.wait().unwrap();
        }
    }

    fn cmd(&self) -> assert_cmd::Command {
        let mut cmd = assert_cmd::Command::new(wobook_bin());
        cmd.arg("--socket").arg(&self.socket);
        cmd.env_remove("WOBOOK_SOCKET");
        cmd
    }

    fn ok(&self, args: &[&str]) -> String {
        let out = self.cmd().args(args).output().unwrap();
        assert!(
            out.status.success(),
            "wobook {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    fn show(&self, url: &str) -> Value {
        serde_json::from_str(&self.ok(&["show", url, "--json"])).unwrap()
    }

    fn hook(&self, name: &str, body: &str) {
        write_exec(&self.hooks.join(name), body);
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.stop();
    }
}

fn tags(v: &Value) -> Vec<String> {
    v["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap().to_string())
        .collect()
}

fn wait_for(path: &Path) -> String {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok(text) = std::fs::read_to_string(path)
            && !text.is_empty()
        {
            return text;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Scripted editor: replaces the line after the marker starting with `marker`.
fn editor_script(dir: &Path, name: &str, marker: &str, line: &str) -> PathBuf {
    let path = dir.join(name);
    write_exec(
        &path,
        &format!("#!/bin/sh\nsed -i '/^{marker}/{{n;s|.*|{line}|;}}' \"$1\"\n"),
    );
    path
}

#[test]
fn core_flow() {
    let d = Daemon::new();
    d.ok(&[
        "add",
        "example.com/x",
        "-t",
        "ui library,react",
        "--no-fetch",
    ]);
    let b = d.show("https://example.com/x");
    assert_eq!(tags(&b), ["react", "ui library"]);
    assert_eq!(b["deleted"], false);
    assert!(
        d.ok(&["show", "https://example.com/x"])
            .contains("react, ui library")
    );

    // Lookup through normalization.
    assert_eq!(d.show("EXAMPLE.com/x#frag")["url"], "https://example.com/x");

    d.ok(&[
        "add",
        "https://ui.shadcn.com/",
        "--title",
        "shadcn/ui",
        "-t",
        "ui,react",
    ]);
    d.ok(&[
        "add",
        "https://vuejs.org/",
        "--title",
        "Vue",
        "-t",
        "ui,vue",
        "--no-fetch",
    ]);
    d.ok(&[
        "add",
        "https://systemd.io/",
        "--title",
        "Init",
        "-t",
        "systemd",
        "--no-fetch",
    ]);

    // TSV shape.
    let list = d.ok(&["list", "--format", "tsv"]);
    assert_eq!(list.lines().count(), 4);
    for line in list.lines() {
        assert_eq!(line.matches('\t').count(), 2, "{line}");
        assert!(line.starts_with("https://"));
    }

    // Search ranking.
    let hits = d.ok(&["search", "shcn", "ui", "--format", "tsv"]);
    assert!(
        hits.lines()
            .next()
            .unwrap()
            .starts_with("https://ui.shadcn.com/"),
        "{hits}"
    );
    assert!(
        d.ok(&["search", "systemd", "--format", "tsv"])
            .contains("https://systemd.io/")
    );

    // Tags and AND filter.
    let t = d.ok(&["tags"]);
    assert!(t.contains("react\t2\n"), "{t}");
    assert!(t.contains("ui library\t1\n"));
    let both = d.ok(&["list", "-t", "react,ui", "--format", "tsv"]);
    assert_eq!(both.lines().count(), 1);
    assert!(both.starts_with("https://ui.shadcn.com/"));

    // Edit tags through a scripted editor.
    let tmp = tempfile::tempdir().unwrap();
    let ed = editor_script(tmp.path(), "tags.sh", "# Add comma-separated TAGS", "a, b");
    d.cmd()
        .env("EDITOR", &ed)
        .args(["edit", "https://vuejs.org/"])
        .assert()
        .success();
    assert_eq!(tags(&d.show("https://vuejs.org/")), ["a", "b"]);

    // Rename via editor.
    let ed = editor_script(
        tmp.path(),
        "url.sh",
        "# Add URL",
        "https://vuejs.org/guide/",
    );
    let out = d
        .cmd()
        .env("EDITOR", &ed)
        .args(["edit", "https://vuejs.org/"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stdout)
            .contains("moved https://vuejs.org/ -> https://vuejs.org/guide/")
    );
    assert_eq!(d.show("https://vuejs.org/")["deleted"], true);
    assert_eq!(d.show("https://vuejs.org/guide/")["title"], "Vue");

    // Editor aborted and unchanged.
    let fail = tmp.path().join("fail.sh");
    write_exec(&fail, "#!/bin/sh\nexit 3\n");
    d.cmd()
        .env("EDITOR", &fail)
        .args(["edit", "https://vuejs.org/guide/"])
        .assert()
        .code(1);
    d.cmd()
        .env("EDITOR", "true")
        .args(["edit", "https://vuejs.org/guide/"])
        .assert()
        .success();

    // mv.
    assert!(
        d.ok(&["mv", "https://vuejs.org/guide/", "vuejs.org/v3"])
            .contains("moved https://vuejs.org/guide/ -> https://vuejs.org/v3")
    );

    // rm and restore.
    d.ok(&["rm", "https://systemd.io/"]);
    assert!(!d.ok(&["list", "--format", "tsv"]).contains("systemd.io"));
    assert!(
        !d.ok(&["search", "Init", "--format", "tsv"])
            .contains("systemd.io")
    );
    assert!(
        d.ok(&["search", "Init", "--format", "tsv", "--include-deleted"])
            .contains("systemd.io")
    );
    d.ok(&["rm", "--restore", "https://systemd.io/"]);
    assert_eq!(tags(&d.show("https://systemd.io/")), ["systemd"]);

    // Duplicate add: exists, then --merge.
    d.cmd()
        .args(["add", "example.com/x", "--no-fetch"])
        .assert()
        .code(1);
    d.ok(&[
        "add",
        "example.com/x",
        "-t",
        "new",
        "--title",
        "X",
        "--merge",
    ]);
    let b = d.show("https://example.com/x");
    assert_eq!(tags(&b), ["new", "react", "ui library"]);
    assert_eq!(b["title"], "X");

    // Not found, invalid input, help.
    d.cmd()
        .args(["show", "https://nope.example/"])
        .assert()
        .code(1);
    d.cmd().args(["add", "not a url at all"]).assert().code(1);
    let help = d.ok(&["--help"]);
    for sub in [
        "add",
        "edit",
        "mv",
        "rm",
        "show",
        "list",
        "search",
        "tags",
        "import",
        "export",
        "status",
        "hooks",
        "completions",
    ] {
        assert!(help.contains(sub), "{sub}");
    }
    d.cmd().args(["bogus"]).assert().code(2);

    // Raw protocol: malformed request and status shape.
    let r = d.request(&json!({"type":"nope"})).unwrap();
    assert_eq!(r["error"]["code"], "invalid_request");
    let s = d.request(&json!({"type":"status"})).unwrap();
    for key in [
        "version",
        "data_dir",
        "socket",
        "bookmark_count",
        "deleted_count",
        "heads",
        "uptime_s",
        "hooks_dir",
    ] {
        assert!(!s["result"][key].is_null(), "{key}");
    }
    let u = d
        .request(&json!({"type":"update","url":"https://example.com/x","add_tags":["z"],"remove_tags":["new"]}))
        .unwrap();
    assert_eq!(tags(&u["result"]), ["react", "ui library", "z"]);
    let r = d
        .request(&json!({"type":"delete","url":"https://missing.example/"}))
        .unwrap();
    assert_eq!(r["error"]["code"], "not_found");
}

#[test]
fn hooks() {
    let d = Daemon::new();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contrib/hooks/pre-add.strip-utm");
    std::fs::copy(&repo, d.hooks.join("pre-add.strip-utm")).unwrap();
    d.ok(&["add", "https://e.example/?a=1&utm_source=x", "--no-fetch"]);
    d.show("https://e.example/?a=1");

    // Log hooks run in order with env and payload.
    let log = d.hooks.join("log.txt");
    for name in ["post-add.10-log", "post-add.20-log"] {
        d.hook(
            name,
            &format!(
                "#!/bin/sh\npayload=$(cat)\necho \"{name} $WOBOOK_EVENT $WOBOOK_ORIGIN $WOBOOK_URL $WOBOOK_DATA_DIR $payload\" >> {}\n",
                log.display()
            ),
        );
    }
    d.hook("post-add.30-fail", "#!/bin/sh\nexit 1\n");
    std::fs::write(d.hooks.join("post-add.40-noexec"), "#!/bin/sh\nexit 1\n").unwrap();
    d.ok(&["add", "https://h.example/", "--title", "H", "--no-fetch"]);
    let deadline = Instant::now() + Duration::from_secs(15);
    let text = loop {
        let text = wait_for(&log);
        if text.lines().count() >= 2 || Instant::now() > deadline {
            break text;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let lines: Vec<&str> = text.lines().collect();
    assert!(
        lines[0].starts_with("post-add.10-log post-add cli https://h.example/ "),
        "{text}"
    );
    assert!(lines[0].contains(&d.data.display().to_string()));
    assert!(lines[1].starts_with("post-add.20-log"));
    let payload: Value = serde_json::from_str(&lines[0][lines[0].find('{').unwrap()..]).unwrap();
    assert_eq!(payload["bookmark"]["title"], "H");
    assert_eq!(payload["previous"], Value::Null);

    // post-update payload has previous.
    let upd = d.hooks.join("upd.txt");
    d.hook(
        "post-update",
        &format!("#!/bin/sh\ncat > {}\n", upd.display()),
    );
    d.request(&json!({"type":"update","url":"https://h.example/","title":"H2"}))
        .unwrap();
    let p: Value = serde_json::from_str(&wait_for(&upd)).unwrap();
    assert_eq!(
        (
            p["bookmark"]["title"].as_str(),
            p["previous"]["title"].as_str()
        ),
        (Some("H2"), Some("H"))
    );

    // Replay.
    let out = d.ok(&["hooks", "run", "post-add", "https://h.example/"]);
    assert!(out.contains("post-add.10-log: exit 0"), "{out}");
    assert!(out.contains("post-add.30-fail: exit 1"));
    assert!(!out.contains("noexec"));
    assert!(d.ok(&["hooks", "list"]).contains("pre-add.strip-utm"));

    // Reject.
    d.hook("pre-add.zz-reject", "#!/bin/sh\necho blocked >&2\nexit 1\n");
    let out = d
        .cmd()
        .args(["add", "https://r.example/", "--no-fetch"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("blocked"));
    let r = d
        .request(&json!({"type":"add","url":"https://r.example/","fetch":false}))
        .unwrap();
    assert_eq!(
        r["error"],
        json!({"code":"hook_rejected","message":"blocked"})
    );
    d.cmd()
        .args(["show", "https://r.example/"])
        .assert()
        .code(1);
}

#[test]
fn import_export() {
    let d = Daemon::new();
    let tmp = tempfile::tempdir().unwrap();

    // buku fixture.
    let db = tmp.path().join("bookmarks.db");
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TABLE bookmarks (id integer PRIMARY KEY, URL text NOT NULL UNIQUE, metadata text default '', tags text default ',', desc text default '', flags integer default 0);
             INSERT INTO bookmarks(URL, metadata, tags, desc) VALUES
               ('https://primevue.org/', 'PrimeVue', ',ui library,vue,', 'Vue UI'),
               ('https://excalidraw.com/', 'Excalidraw', ',drawing,', ''),
               ('https://ai.example/', 'Agent', ',ai agent,', 'd');",
        )
        .unwrap();
    }
    assert!(
        d.ok(&["import", db.to_str().unwrap()])
            .starts_with("added 3 merged 0 skipped 0 errors 0")
    );
    let b = d.show("https://primevue.org/");
    assert_eq!(tags(&b), ["ui library", "vue"]);
    assert_eq!(b["title"], "PrimeVue");
    assert!(
        d.ok(&["import", db.to_str().unwrap()])
            .starts_with("added 0")
    );

    // JSONL roundtrip into a second daemon.
    let jsonl = tmp.path().join("out.jsonl");
    d.ok(&["export", jsonl.to_str().unwrap()]);
    let d2 = Daemon::new();
    assert!(
        d2.ok(&["import", jsonl.to_str().unwrap()])
            .starts_with("added 3")
    );
    let strip = |s: String| {
        let mut v: Vec<Value> = serde_json::from_str(&s).unwrap();
        for x in &mut v {
            x.as_object_mut().unwrap().remove("updated_ms");
        }
        v
    };
    assert_eq!(
        strip(d.ok(&["list", "--format", "json"])),
        strip(d2.ok(&["list", "--format", "json"]))
    );
    assert!(
        d2.ok(&["import", jsonl.to_str().unwrap()])
            .starts_with("added 0 merged 0 skipped 3")
    );

    // Inline export to stdout, deleted omitted.
    d.ok(&["rm", "https://excalidraw.com/"]);
    let inline = d.ok(&["export"]);
    assert_eq!(inline.lines().count(), 2);
    let with_deleted = d.ok(&["export", "--include-deleted"]);
    assert!(with_deleted.contains("\"deleted\":true"));

    // Netscape roundtrip.
    let html = tmp.path().join("out.html");
    d.ok(&["export", html.to_str().unwrap()]);
    assert!(
        std::fs::read_to_string(&html)
            .unwrap()
            .starts_with("<!DOCTYPE NETSCAPE-Bookmark-file-1>")
    );
    let d3 = Daemon::new();
    assert!(
        d3.ok(&["import", html.to_str().unwrap()])
            .starts_with("added 2")
    );
    let b = d3.show("https://primevue.org/");
    assert_eq!(
        (b["title"].as_str(), b["description"].as_str()),
        (Some("PrimeVue"), Some("Vue UI"))
    );
    assert_eq!(tags(&b), ["ui library", "vue"]);

    // Bad line and explicit format.
    let bad = tmp.path().join("bad.txt");
    std::fs::write(
        &bad,
        "{\"url\":\"a.example\"}\n{\"url\":\"b.example\"}\nnot json\n",
    )
    .unwrap();
    d3.cmd()
        .args(["import", bad.to_str().unwrap()])
        .assert()
        .code(1);
    let out = d3
        .cmd()
        .args(["import", bad.to_str().unwrap(), "--format", "jsonl"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stdout).starts_with("added 2 merged 0 skipped 0 errors 1")
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("line 3"));
}

/// Minimal HTTP fixture: serves `response` to every connection.
fn http_fixture(content_type: &'static str, body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().map_while(Result::ok) {
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    format!("http://{addr}/")
}

#[test]
fn durability_and_recovery() {
    let mut d = Daemon::new();
    d.ok(&["add", "https://k.example/", "--no-fetch", "-t", "x"]);
    d.kill9();
    d.start();
    assert_eq!(tags(&d.show("https://k.example/")), ["x"]);

    // Stale socket left by SIGKILL was replaced above; now drop the read model.
    d.stop();
    std::fs::remove_file(d.data.join("read-model.sqlite")).unwrap();
    d.start();
    assert!(
        d.ok(&["list", "--format", "tsv"])
            .contains("https://k.example/")
    );

    // Second daemon refused.
    let out = d.spawn_raw().wait_with_output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("daemon.lock"));
    d.ok(&["status"]);

    // Metadata fetch.
    let ok = http_fixture(
        "text/html; charset=utf-8",
        "<html><head><title>Hello\n  World</title><meta name=\"description\" content=\"Desc\"></head></html>",
    );
    assert!(d.ok(&["add", &ok]).contains("fetch: ok"));
    let b = d.show(&ok);
    assert_eq!(
        (b["title"].as_str(), b["description"].as_str()),
        (Some("Hello World"), Some("Desc"))
    );
    let pdf = http_fixture("application/pdf", "%PDF-1.4");
    assert!(d.ok(&["add", &pdf]).contains("fetch: failed"));
    assert_eq!(d.show(&pdf)["title"], "");
    assert!(
        d.ok(&["add", "http://127.0.0.1:1/"])
            .contains("fetch: failed")
    );
    assert!(
        d.ok(&["add", "https://t.example/", "--title", "T"])
            .contains("fetch: skipped")
    );

    // Daemon down.
    d.stop();
    let out = d.cmd().args(["list"]).output().unwrap();
    assert_eq!(out.status.code(), Some(69));
    assert!(String::from_utf8_lossy(&out.stderr).contains("wobookd is not running"));
}
