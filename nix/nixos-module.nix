# NixOS module: opens the system firewall for wobook peer sync (QUIC on the
# daemon's PORT_RANGE) and mDNS discovery. Daemon, CLI and browser hosts live
# in the home-manager module.
{ config, lib, ... }:
let
  cfg = config.services.wobook;
  rules = {
    allowedUDPPortRanges = [ { from = 47390; to = 47399; } ];
    allowedUDPPorts = [ 5353 ];
  };
in
{
  options.services.wobook = {
    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Allow inbound UDP 47390-47399 (wobook sync) and UDP 5353 (mDNS).";
    };
    firewallInterfaces = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "enp3s0" "tailscale0" ];
      description = "Interfaces to open the ports on. Empty opens them on all interfaces.";
    };
  };

  config = lib.mkIf cfg.openFirewall {
    networking.firewall =
      if cfg.firewallInterfaces == [ ] then rules
      else { interfaces = lib.genAttrs cfg.firewallInterfaces (_: rules); };
  };
}
