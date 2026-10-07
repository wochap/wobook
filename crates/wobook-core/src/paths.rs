//! Default locations (D5, D11).

use std::path::PathBuf;

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// `$WOBOOK_DATA_DIR`, else `$XDG_DATA_HOME/wobook`, else `~/.local/share/wobook`.
pub fn data_dir() -> PathBuf {
    env_path("WOBOOK_DATA_DIR").unwrap_or_else(|| {
        env_path("XDG_DATA_HOME")
            .unwrap_or_else(|| home().join(".local/share"))
            .join("wobook")
    })
}

/// `$WOBOOK_SOCKET`, else `$XDG_RUNTIME_DIR/wobook/wobookd.sock`, else
/// `<data dir>/wobookd.sock`.
pub fn socket_path() -> PathBuf {
    env_path("WOBOOK_SOCKET").unwrap_or_else(|| match env_path("XDG_RUNTIME_DIR") {
        Some(runtime) => runtime.join("wobook").join("wobookd.sock"),
        None => data_dir().join("wobookd.sock"),
    })
}

/// `$WOBOOK_HOOKS_DIR`, else `$XDG_CONFIG_HOME/wobook/hooks`, else
/// `~/.config/wobook/hooks`.
pub fn hooks_dir() -> PathBuf {
    env_path("WOBOOK_HOOKS_DIR").unwrap_or_else(|| {
        env_path("XDG_CONFIG_HOME")
            .unwrap_or_else(|| home().join(".config"))
            .join("wobook")
            .join("hooks")
    })
}
