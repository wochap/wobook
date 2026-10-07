# Shared crane setup: common args and dependency-only artifacts reused by
# every Rust package and check.
{ pkgs, crane, self }:
let
  craneLib = crane.mkLib pkgs;
  commonArgs = {
    # cleanCargoSource plus the crate sources verbatim (READMEs read by
    # include_str!, test fixtures).
    src = pkgs.lib.fileset.toSource {
      root = ./..;
      fileset = pkgs.lib.fileset.unions [
        ../Cargo.toml
        ../Cargo.lock
        ../contrib/hooks # read by the e2e hook test
        (pkgs.lib.fileset.difference ../crates (pkgs.lib.fileset.maybeMissing ../crates/wobook-ffi/target))
      ];
    };
    strictDeps = true;
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = [ pkgs.sqlite ];
    WOBOOK_GIT_REV = self.shortRev or self.dirtyShortRev or "unknown";
  } // { pname = "wobook"; version = (craneLib.crateNameFromCargoToml { cargoToml = ../Cargo.toml; }).version; };
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
{
  inherit craneLib commonArgs cargoArtifacts;
}
