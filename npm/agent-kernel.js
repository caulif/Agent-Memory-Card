#!/usr/bin/env node

const { spawnSync } = require("node:child_process");
const { existsSync } = require("node:fs");
const { join, resolve } = require("node:path");

const root = resolve(__dirname, "..");
const exe = process.platform === "win32" ? "agent-kernel.exe" : "agent-kernel";
const debugPath = join(root, "target", "debug", exe);
const releasePath = join(root, "target", "release", exe);
const bin = existsSync(releasePath) ? releasePath : debugPath;

if (!existsSync(bin)) {
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
}

const result = spawnSync(bin, process.argv.slice(2), {
  cwd: process.cwd(),
  stdio: "inherit"
});

process.exit(result.status ?? 0);
