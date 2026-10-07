# Browser extension bundles. Mirrors extension/esbuild.mjs with the esbuild
# CLI from nixpkgs so no npm dependencies are fetched.
{ lib, runCommand, esbuild, jq, zip }:
let
  src = lib.fileset.toSource {
    root = ../extension;
    fileset = lib.fileset.unions [
      ../extension/manifest.json
      ../extension/icons
      ../extension/src
    ];
  };
  version = (lib.importJSON ../extension/manifest.json).version;
  build = target: manifestFilter: runCommand "wobook-extension-${target}-${version}" {
    nativeBuildInputs = [ esbuild jq ];
  } ''
    mkdir -p $out/popup
    cp -r ${src}/icons $out/icons
    cp ${src}/src/popup/popup.html ${src}/src/popup/popup.css $out/popup/
    jq '${manifestFilter}' ${src}/manifest.json > $out/manifest.json
    cd ${src}
    esbuild src/background.ts --bundle --format=iife --target=firefox121,chrome120 --outfile=$out/background.js
    esbuild src/popup/popup.ts --bundle --format=iife --target=firefox121,chrome120 --outfile=$out/popup/popup.js
  '';
  firefoxDir = build "firefox" ".";
in
{
  extension-chromium = build "chromium" "del(.browser_specific_settings)";
  extension-firefox = runCommand "wobook-extension-firefox-xpi-${version}" {
    nativeBuildInputs = [ zip ];
  } ''
    mkdir -p $out
    cd ${firefoxDir}
    zip -r -X $out/wobook-${version}.xpi .
  '';
}
