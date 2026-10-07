fn main() {
    let rev = std::env::var("WOBOOK_GIT_REV").unwrap_or_else(|_| "unknown".into());
    println!("cargo:rerun-if-env-changed=WOBOOK_GIT_REV");
    println!(
        "cargo:rustc-env=WOBOOK_VERSION={} ({rev})",
        std::env::var("CARGO_PKG_VERSION").unwrap()
    );
}
