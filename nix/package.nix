# Rust packages and the fzf wrapper. Called with callPackage so overlay
# consumers can override inputs.
{ lib, pkgs, rust, installShellFiles, writeShellApplication, fzf, wl-clipboard, xdg-utils, coreutils
, withShellCompletions ? true }:
let
  inherit (rust) craneLib commonArgs cargoArtifacts;

  wobook = craneLib.buildPackage (commonArgs // {
    inherit cargoArtifacts;
    pname = "wobook";
    cargoExtraArgs = "-p wobook";
    doCheck = false;
    nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ installShellFiles ];
    postInstall = lib.optionalString withShellCompletions ''
      installShellCompletion --cmd wobook \
        --zsh <($out/bin/wobook completions zsh) \
        --fish <($out/bin/wobook completions fish)
      mkdir -p $out/share/bash-completion/completions
      $out/bin/wobook completions bash > $out/share/bash-completion/completions/wobook
    '' + ''
      # Native messaging manifests take a path without arguments.
      cat > $out/bin/wobook-native-host <<SH
      #!${pkgs.runtimeShell}
      exec $out/bin/wobook native-host "\$@"
      SH
      chmod +x $out/bin/wobook-native-host
    '';
    meta.mainProgram = "wobook";
  });

  wobookd = craneLib.buildPackage (commonArgs // {
    inherit cargoArtifacts;
    pname = "wobookd";
    cargoExtraArgs = "-p wobookd";
    doCheck = false;
    meta.mainProgram = "wobookd";
  });

  wobook-fzf = writeShellApplication {
    name = "wobook-fzf";
    runtimeInputs = [ wobook fzf wl-clipboard xdg-utils coreutils ];
    text = builtins.readFile ../contrib/wobook-fzf.sh;
  };
in
{
  inherit wobook wobookd wobook-fzf;
}
