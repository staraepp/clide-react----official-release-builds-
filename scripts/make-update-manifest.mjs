// Writes latest.json, the manifest the in-app updater reads.
//
//   node scripts/make-update-manifest.mjs [notes]
//
// Run after `tauri build` (with TAURI_SIGNING_PRIVATE_KEY_PATH set, so the
// update package is signed). Upload these three files to the GitHub release
// tagged v<version>, next to the DMG:
//
//   src-tauri/target/release/bundle/macos/clide.app.tar.gz
//   src-tauri/target/release/bundle/macos/clide.app.tar.gz.sig   (not needed online, but keep it)
//   latest.json                                                    (written here)
//
// The app looks for latest.json at
// https://github.com/staraepp/clide_stt/releases/latest/download/latest.json
// and refuses any package not signed with the matching private key.

import { readFileSync, writeFileSync } from "node:fs";

const version = JSON.parse(readFileSync("package.json", "utf8")).version;
const bundle = "src-tauri/target/release/bundle/macos";
const signature = readFileSync(`${bundle}/clide.app.tar.gz.sig`, "utf8").trim();
const url = `https://github.com/staraepp/clide_stt/releases/download/v${version}/clide.app.tar.gz`;

const platform = { signature, url };
const manifest = {
  version,
  notes: process.argv[2] ?? `clide ${version}`,
  pub_date: new Date().toISOString(),
  platforms: {
    "darwin-aarch64": platform,
    "darwin-aarch64-app": platform,
  },
};

writeFileSync("latest.json", `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`wrote latest.json for v${version}`);
