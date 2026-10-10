# nixos-module Specification

## Purpose
The flake's NixOS module, which opens the system firewall for wobook peer sync and mDNS discovery so pairing works on NixOS hosts.

## Requirements

### Requirement: Module export and options
The flake SHALL export `nixosModules.wobook` and `nixosModules.default` (same module) declaring `services.wobook.openFirewall` (bool, default `true`) and `services.wobook.firewallInterfaces` (list of strings, default `[ ]`). The module SHALL configure only `networking.firewall`; it SHALL NOT install packages or define services.

#### Scenario: Import alone opens the firewall
- **WHEN** a NixOS configuration imports `nixosModules.wobook` and sets no options
- **THEN** UDP 47390-47399 and UDP 5353 are allowed inbound

#### Scenario: Nothing else is added
- **WHEN** the module is imported
- **THEN** `environment.systemPackages` and `systemd.services` contain no wobook entries

### Requirement: Firewall rules
With `openFirewall = true` the module SHALL allow inbound UDP 47390-47399 (the sync port range) and UDP 5353 (mDNS). When `firewallInterfaces` is empty the rules SHALL apply to all interfaces through `networking.firewall.allowedUDPPortRanges` and `networking.firewall.allowedUDPPorts`. When it is non-empty the rules SHALL apply only through `networking.firewall.interfaces.<name>` for each listed interface. With `openFirewall = false` the module SHALL add no firewall rules.

#### Scenario: All interfaces
- **WHEN** `firewallInterfaces = [ ]`
- **THEN** `networking.firewall.allowedUDPPortRanges` contains `{ from = 47390; to = 47399; }` and `networking.firewall.allowedUDPPorts` contains `5353`

#### Scenario: Named interfaces
- **WHEN** `firewallInterfaces = [ "enp3s0" "tailscale0" ]`
- **THEN** both `networking.firewall.interfaces.enp3s0` and `networking.firewall.interfaces.tailscale0` allow UDP 47390-47399 and UDP 5353, and the global allowed lists gain no wobook ports

#### Scenario: Disabled
- **WHEN** `openFirewall = false`
- **THEN** the module adds no ports to any firewall list
