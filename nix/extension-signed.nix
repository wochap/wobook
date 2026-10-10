# Signed Firefox XPI attached to the GitHub release `extension-v<version>`.
# Leave hash empty until the release exists; flake.nix then omits extension-firefox-signed.
{
  version = "0.1.0";
  url = "https://github.com/wochap/wobook/releases/download/extension-v0.1.0/wobook-0.1.0.xpi";
  hash = "";
}
