//! `pair`, `devices`, `sync` and `device` subcommands (p2p-sync D11).

use std::{
    io::{BufRead, IsTerminal, Write},
    time::{Duration, Instant},
};

use serde_json::Value;
use wobook_core::protocol::Request;

use crate::{Ctx, Fail, Result, print};

const POLL: Duration = Duration::from_millis(250);

fn now_s() -> i64 {
    wobook_core::now_ms() / 1000
}

fn ago(ms: Option<i64>) -> String {
    let Some(ms) = ms else {
        return "never".into();
    };
    let s = (wobook_core::now_ms() - ms).max(0) / 1000;
    match s {
        0..60 => format!("{s}s ago"),
        60..3600 => format!("{}m ago", s / 60),
        3600..86400 => format!("{}h ago", s / 3600),
        _ => format!("{}d ago", s / 86400),
    }
}

/// Asks on the terminal (stdin is often the pairing payload pipe).
fn confirm(question: &str) -> bool {
    eprint!("{question} [y/N] ");
    let _ = std::io::stderr().flush();
    let mut answer = String::new();
    let read = match std::fs::File::open("/dev/tty") {
        Ok(tty) => std::io::BufReader::new(tty).read_line(&mut answer),
        Err(_) => std::io::stdin().lock().read_line(&mut answer),
    };
    read.is_ok() && matches!(answer.trim(), "y" | "Y" | "yes")
}

fn session<'a>(pending: &'a Value, id: &str) -> Option<&'a Value> {
    pending
        .as_array()?
        .iter()
        .find(|s| s["session"].as_str() == Some(id))
}

/// Waits for the session to leave `stage`s that are not final; returns it.
fn wait_stage(ctx: &Ctx, id: &str, until: &[&str], deadline: Instant) -> Result<Value> {
    loop {
        let pending = ctx.call(&Request::PairPending)?;
        if let Some(s) = session(&pending, id)
            && let Some(stage) = s["stage"].as_str()
            && (until.contains(&stage) || stage == "failed" || stage == "done")
        {
            return Ok(s.clone());
        }
        if Instant::now() >= deadline {
            return Err(Fail::Input("pairing timed out".into()));
        }
        std::thread::sleep(POLL);
    }
}

fn failed(s: &Value) -> Fail {
    match s["error"].as_str().unwrap_or("protocol") {
        "rejected" => Fail::Input("pairing rejected; nothing was shared".into()),
        code => Fail::Input(format!("pairing failed: {code}")),
    }
}

/// Shows the peer, asks (or `--yes`), sends the decision and waits for the end.
fn decide(ctx: &Ctx, s: &Value, yes: bool) -> Result<()> {
    let id = s["session"].as_str().unwrap_or_default().to_string();
    if s["stage"] == "failed" {
        return Err(failed(s));
    }
    eprintln!(
        "Peer: {} ({})\nFingerprint: {}",
        s["peer_name"].as_str().unwrap_or("?"),
        s["peer_platform"].as_str().unwrap_or("?"),
        s["fingerprint"].as_str().unwrap_or("?")
    );
    let trusted = yes || confirm("Trust this device?");
    let request = if trusted {
        Request::PairConfirm {
            session: id.clone(),
        }
    } else {
        Request::PairReject {
            session: id.clone(),
        }
    };
    match ctx.call(&request) {
        Ok(_) => {}
        // The session ended meanwhile (bad proof, peer rejected): report why.
        Err(Fail::Client(crate::client::ClientError::Remote(
            wobook_core::protocol::ErrorCode::PairPendingMissing,
            _,
        ))) => {
            let pending = ctx.call(&Request::PairPending)?;
            return Err(session(&pending, &id)
                .map_or_else(|| Fail::Input("pairing session vanished".into()), failed));
        }
        Err(e) => return Err(e),
    }
    if !trusted {
        return Err(Fail::Input("pairing rejected; nothing was shared".into()));
    }
    let done = wait_stage(ctx, &id, &[], Instant::now() + Duration::from_secs(150))?;
    if done["stage"] == "done" {
        eprintln!("paired with {}", done["peer_name"].as_str().unwrap_or("?"));
        Ok(())
    } else {
        Err(failed(&done))
    }
}

pub fn pair(
    ctx: &Ctx,
    join: Option<String>,
    yes: bool,
    json: bool,
    name: Option<String>,
) -> Result<()> {
    if let Some(name) = name {
        ctx.call(&Request::DeviceName { name: Some(name) })?;
    }
    if let Some(join) = join {
        let text = if join == "-" {
            let mut line = String::new();
            std::io::stdin()
                .lock()
                .read_line(&mut line)
                .map_err(|e| Fail::Input(e.to_string()))?;
            line
        } else {
            join
        };
        let payload: Value = serde_json::from_str(text.trim())
            .map_err(|e| Fail::Input(format!("pairing payload is not JSON: {e}")))?;
        let started = ctx.call(&Request::PairJoin { payload })?;
        let id = started["session"].as_str().unwrap_or_default().to_string();
        let s = wait_stage(
            ctx,
            &id,
            &["confirming"],
            Instant::now() + Duration::from_secs(60),
        )?;
        return decide(ctx, &s, yes);
    }
    let started = ctx.call(&Request::PairStart)?;
    let payload = started["payload"].to_string();
    if json {
        print(&format!("{payload}\n"));
    } else {
        print(&format!(
            "{}\n{payload}\n",
            started["qr_text"].as_str().unwrap_or("")
        ));
        if std::io::stderr().is_terminal() {
            eprintln!("Scan the code or paste the JSON into `wobook pair --join -`; valid 120 s.");
        }
    }
    let _ = std::io::stdout().flush();
    let before: Vec<String> = ctx
        .call(&Request::PairPending)?
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s["session"].as_str().map(str::to_string))
        .collect();
    let expires = started["expires_at"].as_i64().unwrap_or(now_s() + 120);
    loop {
        let pending = ctx.call(&Request::PairPending)?;
        let found = pending.as_array().into_iter().flatten().find(|s| {
            s["role"] == "offerer"
                && (s["stage"] == "confirming"
                    || (s["stage"] == "failed" && s["error"] == "rejected"))
                && !before.iter().any(|b| s["session"].as_str() == Some(b))
        });
        if let Some(s) = found {
            return decide(ctx, &s.clone(), yes);
        }
        if now_s() > expires + 2 {
            return Err(Fail::Input("pairing window expired".into()));
        }
        std::thread::sleep(POLL);
    }
}

fn devices(ctx: &Ctx) -> Result<Vec<Value>> {
    Ok(ctx
        .call(&Request::DevicesList)?
        .as_array()
        .cloned()
        .unwrap_or_default())
}

pub fn devices_list(ctx: &Ctx, json: bool) -> Result<()> {
    let list = ctx.call(&Request::DevicesList)?;
    if json {
        print(&format!("{list}\n"));
        return Ok(());
    }
    for d in list.as_array().into_iter().flatten() {
        let dot = match d["reachability"].as_str() {
            Some("lan") => "● lan",
            Some("tailnet") => "● tailnet",
            _ => "○ unreachable",
        };
        let endpoints: Vec<&str> = d["endpoints"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| e["address"].as_str())
            .collect();
        print(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}{}\n",
            d["name"].as_str().unwrap_or(""),
            d["platform"].as_str().unwrap_or(""),
            dot,
            ago(d["last_sync_ms"].as_i64()),
            endpoints.join(","),
            &d["id"].as_str().unwrap_or("")[..12.min(d["id"].as_str().unwrap_or("").len())],
            if d["revoked"] == true {
                "\trevoked"
            } else {
                ""
            }
        ));
    }
    Ok(())
}

pub fn devices_revoke(ctx: &Ctx, device: String, yes: bool) -> Result<()> {
    let lower = device.to_lowercase();
    let name = devices(ctx)?
        .into_iter()
        .find(|d| {
            d["id"].as_str().is_some_and(|id| id.starts_with(&lower))
                || d["name"]
                    .as_str()
                    .is_some_and(|n| n.to_lowercase() == lower)
        })
        .and_then(|d| d["name"].as_str().map(str::to_string))
        .unwrap_or_else(|| device.clone());
    if !yes && !confirm(&format!("Revoke {name}? This cannot be undone.")) {
        return Err(Fail::Input("not revoked".into()));
    }
    let result = ctx.call(&Request::DevicesRevoke { id: device })?;
    print(&format!(
        "revoked {name} ({})\n",
        result["id"].as_str().unwrap_or("")
    ));
    Ok(())
}

pub fn sync_status(ctx: &Ctx, json: bool) -> Result<()> {
    let status = ctx.call(&Request::SyncStatus)?;
    if json {
        print(&format!("{status}\n"));
        return Ok(());
    }
    let d = &status["device"];
    print(&format!(
        "{} (this device)\t{}\tport {}\n",
        d["name"].as_str().unwrap_or(""),
        &d["id"].as_str().unwrap_or("")[..12.min(d["id"].as_str().unwrap_or("").len())],
        d["port"]
    ));
    for p in status["peers"].as_array().into_iter().flatten() {
        let state = if p["connected"] == true {
            if p["heads_equal"] == true {
                "synced"
            } else {
                "syncing"
            }
        } else {
            p["last_error"].as_str().unwrap_or("offline")
        };
        print(&format!(
            "{}\t{}\t{}\tlast sync {}\n",
            p["name"].as_str().unwrap_or(""),
            p["reachability"].as_str().unwrap_or(""),
            state,
            ago(p["last_sync_ms"].as_i64())
        ));
    }
    Ok(())
}
