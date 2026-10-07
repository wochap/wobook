# wobook module for nix-config, same shape as programs/cli/buku.
#
# Setup:
# 1. flake.nix inputs:
#      wobook.url = "github:wochap/wobook";
#      wobook.inputs.nixpkgs.follows = "nixpkgs";
#      wobook.inputs.home-manager.follows = "home-manager";
# 2. Copy this file to modules/shared/programs/cli/wobook/default.nix and add
#    ./programs/cli/wobook to the imports in modules/shared/default.nix.
# 3. Per host (e.g. hosts/gdesktop/default.nix):
#      _custom.programs.buku.enable = false;
#      _custom.programs.wobook.enable = true;
# 4. In tui-bookmarks.sh swap `buku-fzf {}` for `wobook-fzf {}`.
#
# If `_custom.hm` does not accept `imports`, drop the `imports` line below and
# import the module once instead:
#      home-manager.sharedModules = [ inputs.wobook.homeManagerModules.wobook ];
{ config, lib, inputs, ... }:

let cfg = config._custom.programs.wobook;
in {
  options._custom.programs.wobook.enable = lib.mkEnableOption { };

  config = lib.mkIf cfg.enable {
    _custom.hm = {
      imports = [ inputs.wobook.homeManagerModules.wobook ];

      programs.wobook = {
        enable = true;
        daemon.enable = true;
        fzf.enable = true;
        deviceName = config.networking.hostName;
        hooks."pre-add.strip-utm" = "${inputs.wobook}/contrib/hooks/pre-add.strip-utm";
        browsers = {
          firefox.enable = true;
          googleChrome.enable = true;
          brave.enable = true;
          # Id shown on chrome://extensions after "Load unpacked".
          chromiumExtensionIds = [ "REPLACE_WITH_EXTENSION_ID" ];
        };
      };
    };
  };
}
