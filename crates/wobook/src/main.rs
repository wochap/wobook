//! wobook: CLI client of wobookd.

mod client;
mod editor;
mod native_host;
mod output;
mod sync_cmd;

use std::{
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{CommandFactory, Parser, Subcommand};
use serde_json::Value;
use wobook_core::{
    interchange::Format,
    model::Bookmark,
    protocol::{ErrorCode, Request},
};

use crate::{
    client::ClientError,
    output::{OutputFormat, render},
};

#[derive(Parser, Debug)]
#[command(name = "wobook", version = env!("WOBOOK_VERSION"), about = "Local-first bookmarks")]
struct Cli {
    /// Daemon socket (default: $WOBOOK_SOCKET or $XDG_RUNTIME_DIR/wobook/wobookd.sock).
    #[arg(long, global = true)]
    socket: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum FileFormat {
    Jsonl,
    Netscape,
    Buku,
}

impl From<FileFormat> for Format {
    fn from(f: FileFormat) -> Self {
        match f {
            FileFormat::Jsonl => Format::Jsonl,
            FileFormat::Netscape => Format::Netscape,
            FileFormat::Buku => Format::Buku,
        }
    }
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum TagsFormat {
    Tsv,
    Json,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Add a bookmark.
    Add {
        url: String,
        /// Comma-separated tags.
        #[arg(short, long)]
        tags: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long = "desc")]
        description: Option<String>,
        /// Do not fetch title and description.
        #[arg(long)]
        no_fetch: bool,
        /// Merge into an existing bookmark instead of failing.
        #[arg(long)]
        merge: bool,
    },
    /// Edit a bookmark in $EDITOR (changing the URL moves it).
    Edit {
        #[arg(required_unless_present = "new")]
        url: Option<String>,
        /// Open an empty template and add the result.
        #[arg(long, conflicts_with = "url")]
        new: bool,
    },
    /// Move a bookmark to a new URL.
    Mv { old: String, new: String },
    /// Delete (tombstone) bookmarks, or restore them.
    Rm {
        #[arg(required = true)]
        urls: Vec<String>,
        #[arg(long)]
        restore: bool,
    },
    /// Show one bookmark.
    Show {
        url: String,
        #[arg(long)]
        json: bool,
    },
    /// List bookmarks, newest first.
    List {
        /// Only bookmarks carrying all these comma-separated tags.
        #[arg(short, long)]
        tags: Option<String>,
        #[arg(long, value_enum)]
        format: Option<OutputFormat>,
        #[arg(long)]
        include_deleted: bool,
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Fuzzy search.
    Search {
        query: Vec<String>,
        #[arg(short, long)]
        tags: Option<String>,
        #[arg(long, value_enum)]
        format: Option<OutputFormat>,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        include_deleted: bool,
    },
    /// List tags with counts.
    Tags {
        #[arg(long, value_enum, default_value = "tsv")]
        format: TagsFormat,
    },
    /// Import bookmarks (format inferred from .jsonl/.html/.db).
    Import {
        path: PathBuf,
        #[arg(long, value_enum)]
        format: Option<FileFormat>,
    },
    /// Export bookmarks to a file or stdout.
    Export {
        path: Option<PathBuf>,
        #[arg(long, value_enum)]
        format: Option<FileFormat>,
        #[arg(long)]
        include_deleted: bool,
    },
    /// Daemon status.
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Inspect and replay hooks.
    Hooks {
        #[command(subcommand)]
        command: HooksCommand,
    },
    /// Print shell completions.
    Completions { shell: clap_complete::Shell },
    /// Pair with another device: show a QR code, or join with a payload.
    Pair {
        /// Join using the payload JSON from the other device ("-" reads stdin).
        #[arg(long)]
        join: Option<String>,
        /// Trust without prompting (scripted setups; weaker).
        #[arg(long)]
        yes: bool,
        /// Print only the payload JSON.
        #[arg(long)]
        json: bool,
        /// Set this device's name first.
        #[arg(long)]
        name: Option<String>,
    },
    /// Trusted devices.
    Devices {
        #[command(subcommand)]
        command: DevicesCommand,
    },
    /// Synchronization state.
    Sync {
        #[command(subcommand)]
        command: SyncCommand,
    },
    /// Native messaging host for the browser extension (stdio framing).
    NativeHost {
        /// Print the host manifest for a browser and exit.
        #[arg(long, value_enum)]
        print_manifest: Option<native_host::Browser>,
        /// Extension id (required for chrome and brave).
        #[arg(long, requires = "print_manifest")]
        extension_id: Option<String>,
        /// Absolute path of the host executable written into the manifest.
        #[arg(long, requires = "print_manifest")]
        binary: Option<PathBuf>,
    },
    /// This device.
    Device {
        #[command(subcommand)]
        command: DeviceCommand,
    },
}

#[derive(Subcommand, Debug)]
enum DevicesCommand {
    /// List trusted and revoked devices.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Rename a device locally.
    Rename { device: String, name: String },
    /// Revoke a device permanently.
    Revoke {
        device: String,
        #[arg(long)]
        yes: bool,
    },
    /// Remember an address for a device.
    AddEndpoint { device: String, address: String },
}

#[derive(Subcommand, Debug)]
enum SyncCommand {
    /// Per-peer reachability and last sync.
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Connect to every trusted device now.
    Now,
}

#[derive(Subcommand, Debug)]
enum DeviceCommand {
    /// Show or set this device's name.
    Name { name: Option<String> },
}

#[derive(Subcommand, Debug)]
enum HooksCommand {
    /// List installed hooks.
    List,
    /// Run the hooks of <event> with the current record of <url>.
    Run { event: String, url: String },
}

pub(crate) enum Fail {
    Client(ClientError),
    Input(String),
    Internal(String),
}

impl From<ClientError> for Fail {
    fn from(e: ClientError) -> Self {
        Self::Client(e)
    }
}

pub(crate) type Result<T> = std::result::Result<T, Fail>;

pub(crate) struct Ctx {
    socket: PathBuf,
}

impl Ctx {
    pub(crate) fn call(&self, request: &Request) -> Result<Value> {
        Ok(client::call(&self.socket, request)?)
    }
    fn get(&self, url: &str) -> Result<Bookmark> {
        parse(self.call(&Request::Get {
            url: url.to_string(),
        })?)
    }
}

fn parse<T: serde::de::DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|e| Fail::Internal(e.to_string()))
}

fn split_tags(tags: Option<&str>) -> Vec<String> {
    tags.map(|t| t.split(',').map(str::to_string).collect())
        .unwrap_or_default()
}

fn absolute(path: &Path) -> Result<String> {
    std::path::absolute(path)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| Fail::Input(e.to_string()))
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let ctx = Ctx {
        socket: cli.socket.unwrap_or_else(wobook_core::paths::socket_path),
    };
    if let Command::NativeHost {
        print_manifest,
        extension_id,
        binary,
    } = cli.command
    {
        return match print_manifest {
            None => native_host::run(&ctx.socket),
            Some(browser) => match native_host::print_manifest(browser, extension_id, binary) {
                Ok(()) => ExitCode::SUCCESS,
                Err(message) => Cli::command()
                    .error(clap::error::ErrorKind::MissingRequiredArgument, message)
                    .exit(),
            },
        };
    }
    match run(&ctx, cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(fail) => {
            let (code, message) = match fail {
                Fail::Client(ClientError::Unavailable) => (
                    69,
                    "wobookd is not running (start it: wobookd, or systemctl --user start wobookd)"
                        .to_string(),
                ),
                Fail::Client(ClientError::Remote(code, message)) => match code {
                    ErrorCode::Io | ErrorCode::Internal => (70, message),
                    _ => (1, message),
                },
                Fail::Client(ClientError::Internal(m)) | Fail::Internal(m) => (70, m),
                Fail::Input(m) => (1, m),
            };
            eprintln!("wobook: {message}");
            ExitCode::from(code)
        }
    }
}

pub(crate) fn print(text: &str) {
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(text.as_bytes());
}

fn run(ctx: &Ctx, command: Command) -> Result<()> {
    match command {
        Command::Add {
            url,
            tags,
            title,
            description,
            no_fetch,
            merge,
        } => {
            let result = ctx.call(&Request::Add {
                url,
                title,
                description,
                tags: split_tags(tags.as_deref()),
                fetch: !no_fetch,
                merge,
                origin: "cli".into(),
            })?;
            report_add(&result);
        }
        Command::Edit { url: Some(url), .. } => edit_existing(ctx, &url)?,
        Command::Edit { url: None, .. } => edit_new(ctx)?,
        Command::Mv { old, new } => {
            let result = ctx.call(&Request::Rename {
                from: old,
                to: new,
                origin: "cli".into(),
            })?;
            print(&format!(
                "moved {} -> {}\n",
                result["from"].as_str().unwrap_or(""),
                result["to"].as_str().unwrap_or("")
            ));
        }
        Command::Rm { urls, restore } => {
            for url in urls {
                let request = if restore {
                    Request::Restore {
                        url,
                        origin: "cli".into(),
                    }
                } else {
                    Request::Delete {
                        url,
                        origin: "cli".into(),
                    }
                };
                let b: Bookmark = parse(ctx.call(&request)?)?;
                print(&format!(
                    "{} {}\n",
                    if restore { "restored" } else { "deleted" },
                    b.url
                ));
            }
        }
        Command::Show { url, json } => {
            let b = ctx.get(&url)?;
            if json {
                print(&format!(
                    "{}\n",
                    serde_json::to_string(&b).unwrap_or_default()
                ));
            } else {
                print(&output::pretty(&b));
            }
        }
        Command::List {
            tags,
            format,
            include_deleted,
            limit,
        } => {
            let list: Vec<Bookmark> = parse(ctx.call(&Request::List {
                tags: split_tags(tags.as_deref()),
                include_deleted,
                limit,
            })?)?;
            print(&render(&list, OutputFormat::resolve(format)));
        }
        Command::Search {
            query,
            tags,
            format,
            limit,
            include_deleted,
        } => {
            let hits: Vec<Value> = parse(ctx.call(&Request::Search {
                query: query.join(" "),
                tags: split_tags(tags.as_deref()),
                include_deleted,
                limit,
            })?)?;
            let list = hits
                .into_iter()
                .map(|mut h| parse::<Bookmark>(h["bookmark"].take()))
                .collect::<Result<Vec<_>>>()?;
            print(&render(&list, OutputFormat::resolve(format)));
        }
        Command::Tags { format } => {
            let tags = ctx.call(&Request::Tags)?;
            match format {
                TagsFormat::Json => print(&format!("{tags}\n")),
                TagsFormat::Tsv => {
                    for t in tags.as_array().into_iter().flatten() {
                        print(&format!(
                            "{}\t{}\n",
                            t["tag"].as_str().unwrap_or(""),
                            t["count"]
                        ));
                    }
                }
            }
        }
        Command::Import { path, format } => {
            let result = ctx.call(&Request::Import {
                format: format.map(Into::into),
                path: absolute(&path)?,
            })?;
            print(&format!(
                "added {} merged {} skipped {} errors {}\n",
                result["added"],
                result["merged"],
                result["skipped"],
                result["errors"].as_array().map_or(0, Vec::len)
            ));
            for e in result["errors"].as_array().into_iter().flatten() {
                eprintln!(
                    "line {}: {}",
                    e["line"],
                    e["message"].as_str().unwrap_or("")
                );
            }
        }
        Command::Export {
            path,
            format,
            include_deleted,
        } => {
            let format = format
                .map(Into::into)
                .or_else(|| path.as_deref().and_then(Format::infer))
                .unwrap_or(Format::Jsonl);
            let result = ctx.call(&Request::Export {
                format,
                path: path.as_deref().map(absolute).transpose()?,
                include_deleted,
            })?;
            match result["content"].as_str() {
                Some(content) => print(content),
                None => eprintln!(
                    "exported {} to {}",
                    result["count"],
                    result["path"].as_str().unwrap_or("")
                ),
            }
        }
        Command::Status { json } => {
            let status = ctx.call(&Request::Status)?;
            if json {
                print(&format!("{status}\n"));
            } else if let Some(map) = status.as_object() {
                for (k, v) in map {
                    print(&format!(
                        "{k}: {}\n",
                        v.as_str().map_or_else(|| v.to_string(), str::to_string)
                    ));
                }
            }
        }
        Command::Hooks {
            command: HooksCommand::List,
        } => {
            for h in ctx.call(&Request::Hooks)?.as_array().into_iter().flatten() {
                print(&format!(
                    "{}\t{}\n",
                    h["event"].as_str().unwrap_or(""),
                    h["path"].as_str().unwrap_or("")
                ));
            }
        }
        Command::Hooks {
            command: HooksCommand::Run { event, url },
        } => {
            for o in ctx
                .call(&Request::RunHooks { event, url })?
                .as_array()
                .into_iter()
                .flatten()
            {
                let code = if o["timed_out"].as_bool() == Some(true) {
                    "timeout".to_string()
                } else {
                    o["exit_code"].to_string()
                };
                print(&format!(
                    "{}: exit {code}\n",
                    o["hook"].as_str().unwrap_or("")
                ));
                for stream in ["stdout", "stderr"] {
                    let text = o[stream].as_str().unwrap_or("").trim_end();
                    if !text.is_empty() {
                        print(&format!("  {stream}: {}\n", text.replace('\n', "\n  ")));
                    }
                }
            }
        }
        Command::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "wobook", &mut std::io::stdout());
        }
        Command::Pair {
            join,
            yes,
            json,
            name,
        } => sync_cmd::pair(ctx, join, yes, json, name)?,
        Command::Devices { command } => match command {
            DevicesCommand::List { json } => sync_cmd::devices_list(ctx, json)?,
            DevicesCommand::Rename { device, name } => {
                let r = ctx.call(&Request::DevicesRename { id: device, name })?;
                print(&format!(
                    "renamed {} to {}\n",
                    r["id"].as_str().unwrap_or(""),
                    r["name"].as_str().unwrap_or("")
                ));
            }
            DevicesCommand::Revoke { device, yes } => sync_cmd::devices_revoke(ctx, device, yes)?,
            DevicesCommand::AddEndpoint { device, address } => {
                ctx.call(&Request::DevicesAddEndpoint {
                    id: device,
                    address: address.clone(),
                })?;
                print(&format!("added {address}\n"));
            }
        },
        Command::Sync {
            command: SyncCommand::Status { json },
        } => sync_cmd::sync_status(ctx, json)?,
        Command::Sync {
            command: SyncCommand::Now,
        } => {
            ctx.call(&Request::SyncNow)?;
        }
        Command::NativeHost { .. } => unreachable!("dispatched in main"),
        Command::Device {
            command: DeviceCommand::Name { name },
        } => {
            let r = ctx.call(&Request::DeviceName { name })?;
            print(&format!("{}\n", r["name"].as_str().unwrap_or("")));
        }
    }
    Ok(())
}

fn report_add(result: &Value) {
    let url = result["bookmark"]["url"].as_str().unwrap_or("");
    let verb = if result["created"].as_bool() == Some(true) {
        "added"
    } else if result["restored"].as_bool() == Some(true) {
        "restored"
    } else {
        "merged"
    };
    print(&format!(
        "{verb} {url} (fetch: {})\n",
        result["fetch"].as_str().unwrap_or("skipped")
    ));
}

/// Runs the editor on `initial`; `None` when the content is unchanged.
fn run_editor(initial: &str) -> Result<Option<String>> {
    let mut file = tempfile::Builder::new()
        .prefix("wobook-")
        .suffix(".txt")
        .tempfile()
        .map_err(|e| Fail::Internal(e.to_string()))?;
    file.write_all(initial.as_bytes())
        .map_err(|e| Fail::Internal(e.to_string()))?;
    file.flush().map_err(|e| Fail::Internal(e.to_string()))?;
    let editor = ["EDITOR", "VISUAL"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.trim().is_empty()))
        .unwrap_or_else(|| "vi".into());
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("sh")
        .arg(file.path())
        .status()
        .map_err(|e| Fail::Input(format!("cannot run editor: {e}")))?;
    if !status.success() {
        return Err(Fail::Input(format!(
            "editor exited with {status}; nothing changed"
        )));
    }
    let text = std::fs::read_to_string(file.path()).map_err(|e| Fail::Internal(e.to_string()))?;
    Ok((text != initial).then_some(text))
}

fn edit_existing(ctx: &Ctx, url: &str) -> Result<()> {
    let current = ctx.get(url)?;
    let Some(text) = run_editor(&editor::render(Some(&current)))? else {
        eprintln!("no changes");
        return Ok(());
    };
    let edited = editor::parse(&text);
    if edited.url.is_empty() {
        return Err(Fail::Input("URL is empty; nothing changed".into()));
    }
    let mut target = current.url.clone();
    let new_url = wobook_core::url::normalize(&edited.url)
        .map_err(|e| Fail::Input(e.to_string()))?
        .into_string();
    if new_url != current.url {
        let result = ctx.call(&Request::Rename {
            from: current.url.clone(),
            to: new_url,
            origin: "cli".into(),
        })?;
        target = result["to"].as_str().unwrap_or_default().to_string();
        print(&format!("moved {} -> {target}\n", current.url));
    }
    let title = edited.title.unwrap_or_default();
    let mut tags = edited.tags;
    tags.sort();
    let title = (title != current.title).then_some(title);
    let description = (edited.description != current.description).then_some(edited.description);
    let tags = (tags != current.tags).then_some(tags);
    if title.is_some() || description.is_some() || tags.is_some() {
        ctx.call(&Request::Update {
            url: target,
            title,
            description,
            tags,
            add_tags: None,
            remove_tags: None,
            origin: "cli".into(),
        })?;
    }
    Ok(())
}

fn edit_new(ctx: &Ctx) -> Result<()> {
    let Some(text) = run_editor(&editor::render(None))? else {
        eprintln!("no changes");
        return Ok(());
    };
    let edited = editor::parse(&text);
    if edited.url.is_empty() {
        return Err(Fail::Input("URL is empty; nothing added".into()));
    }
    let fetch = edited.title.is_none();
    let result = ctx.call(&Request::Add {
        url: edited.url,
        title: edited.title,
        description: (!edited.description.is_empty()).then_some(edited.description),
        tags: edited.tags,
        fetch,
        merge: true,
        origin: "cli".into(),
    })?;
    report_add(&result);
    Ok(())
}
