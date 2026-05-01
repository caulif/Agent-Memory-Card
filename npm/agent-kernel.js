#!/usr/bin/env node

const { spawnSync } = require("node:child_process");
const { existsSync } = require("node:fs");
const { join, resolve } = require("node:path");
const { findExistingBinary, resolveBinaryCandidates } = require("./agent-kernel-lib");

const root = resolve(__dirname, "..");
let bin = findExistingBinary({ root });

if (!bin) {
  const homeCargo = process.env.USERPROFILE
    ? join(process.env.USERPROFILE, ".cargo", "bin", process.platform === "win32" ? "cargo.exe" : "cargo")
    : null;
  const cargo = process.env.CARGO || (homeCargo && existsSync(homeCargo) ? homeCargo : "cargo");
  const built = spawnSync(cargo, ["build"], {
    cwd: root,
    stdio: "inherit",
    shell: process.platform === "win32"
  });
  if (built.status !== 0) {
    process.exit(built.status ?? 1);
  }
  bin = findExistingBinary({ root });
}

if (!bin) {
  console.error("agent-kernel binary was not found after build.");
  console.error("Checked:");
  for (const candidate of resolveBinaryCandidates({ root })) {
    console.error(`- ${candidate}`);
  }
  process.exit(1);
}

const result = spawnSync(bin, process.argv.slice(2), {
  cwd: process.cwd(),
  stdio: "inherit"
});

process.exit(result.status ?? 0);
