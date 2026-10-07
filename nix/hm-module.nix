# Home-manager module: programs.wobook.
{ self }:
{ config, lib, pkgs, ... }:
let
  cfg = config.programs.wobook;
  inherit (lib) mkEnableOption mkOption mkIf types optional optionalAttrs;
  system = pkgs.stdenv.hostPlatform.system;
  hostName = "dev.wochap.wobook";

  cliPackage =
    if cfg.shellCompletions.enable then cfg.package
    else pkgs.symlinkJoin {
      name = "wobook-bin-only";
      paths = [ cfg.package ];
      postBuild = "find $out -mindepth 1 -maxdepth 1 ! -name bin -exec rm -rf {} +";
    };
  fzfPackage = self.packages.${system}.wobook-fzf;

  nativeHost = "${cfg.package}/bin/wobook-native-host";
  manifestBase = {
    name = hostName;
    description = "wobook native messaging host";
    path = nativeHost;
    type = "stdio";
  };
  firefoxManifest = builtins.toJSON (manifestBase // {
    allowed_extensions = [ cfg.browsers.firefoxExtensionId ];
  });
  chromiumManifest = builtins.toJSON (manifestBase // {
    allowed_origins = map (id: "chrome-extension://${id}/") cfg.browsers.chromiumExtensionIds;
  });
  chromiumDirs =
    optional cfg.browsers.googleChrome.enable "google-chrome"
    ++ optional cfg.browsers.brave.enable "BraveSoftware/Brave-Browser"
    ++ cfg.browsers.extraChromiumDirs;

  env =
    optionalAttrs (cfg.dataDir != null) { WOBOOK_DATA_DIR = cfg.dataDir; }
    // optionalAttrs (cfg.deviceName != null) { WOBOOK_DEVICE_NAME = cfg.deviceName; };
in
{
  options.programs.wobook = {
    enable = mkEnableOption "wobook, local-first bookmarks";
    package = mkOption {
      type = types.package;
      default = self.packages.${system}.wobook;
      description = "Package providing bin/wobook and bin/wobook-native-host.";
    };
    daemonPackage = mkOption {
      type = types.package;
      default = self.packages.${system}.wobookd;
      description = "Package providing bin/wobookd.";
    };
    deviceName = mkOption {
      type = types.nullOr types.str;
      default = null;
      description = "Device name shown to peers (WOBOOK_DEVICE_NAME).";
    };
    dataDir = mkOption {
      type = types.nullOr types.str;
      default = null;
      description = "Data directory (WOBOOK_DATA_DIR).";
    };
    daemon.enable = mkOption {
      type = types.bool;
      default = true;
      description = "Run wobookd as a systemd user service.";
    };
    daemon.extraArgs = mkOption {
      type = types.listOf types.str;
      default = [ ];
      description = "Extra arguments passed to wobookd.";
    };
    hooks = mkOption {
      type = types.attrsOf (types.either types.str types.path);
      default = { };
      description = "Hooks installed executable under $XDG_CONFIG_HOME/wobook/hooks/<name>; value is inline text or a path.";
    };
    fzf.enable = mkEnableOption "the wobook-fzf wrapper";
    shellCompletions.enable = mkOption {
      type = types.bool;
      default = true;
      description = "Install zsh, fish and bash completions.";
    };
    browsers = {
      firefox.enable = mkEnableOption "the Firefox native messaging host manifest";
      googleChrome.enable = mkEnableOption "the Google Chrome native messaging host manifest";
      brave.enable = mkEnableOption "the Brave native messaging host manifest";
      firefoxExtensionId = mkOption {
        type = types.str;
        default = "wobook@wochap.dev";
        description = "Firefox extension id allowed to talk to the host.";
      };
      chromiumExtensionIds = mkOption {
        type = types.listOf types.str;
        default = [ ];
        description = "Chromium extension ids allowed to talk to the host (shown on chrome://extensions after Load unpacked).";
      };
      extraChromiumDirs = mkOption {
        type = types.listOf types.str;
        default = [ ];
        example = [ "chromium" ];
        description = "Extra Chromium-family config dirs relative to ~/.config that receive the manifest.";
      };
    };
  };

  config = lib.mkMerge [
    {
      assertions = [
        {
          assertion = cfg.fzf.enable -> cfg.enable;
          message = "programs.wobook.fzf.enable requires programs.wobook.enable";
        }
      ];
    }
    (mkIf cfg.enable {
      assertions = [
        {
          assertion = chromiumDirs == [ ] || cfg.browsers.chromiumExtensionIds != [ ];
          message = "programs.wobook.browsers.chromiumExtensionIds must be non-empty when a Chromium-family browser is enabled";
        }
      ];

      home.packages = [ cliPackage ] ++ optional cfg.fzf.enable fzfPackage;
      home.sessionVariables = env;

      systemd.user.services.wobookd = mkIf cfg.daemon.enable {
        Unit.Description = "wobook daemon";
        Service = {
          ExecStart = "${cfg.daemonPackage}/bin/wobookd ${lib.escapeShellArgs cfg.daemon.extraArgs}";
          Restart = "on-failure";
          RestartSec = 2;
          Environment = lib.mapAttrsToList (k: v: "${k}=${v}") env;
        };
        Install.WantedBy = [ "default.target" ];
      };

      xdg.configFile = lib.mapAttrs' (name: value: lib.nameValuePair "wobook/hooks/${name}" (
        { executable = true; }
        // (if builtins.isPath value || lib.hasPrefix "/" value then { source = value; } else { text = value; })
      )) cfg.hooks;

      home.file =
        optionalAttrs cfg.browsers.firefox.enable {
          ".mozilla/native-messaging-hosts/${hostName}.json".text = firefoxManifest;
        }
        // lib.listToAttrs (map (dir: lib.nameValuePair ".config/${dir}/NativeMessagingHosts/${hostName}.json" {
          text = chromiumManifest;
        }) chromiumDirs);
    })
  ];
}
