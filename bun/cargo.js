#!/usr/bin/env bun

const { spawnSync } = require("node:child_process");
const { existsSync } = require("node:fs");
const { join } = require("node:path");

function cargoCandidates() {
  const exe = process.platform === "win32" ? "cargo.exe" : "cargo";
  const candidates = [];

  if (process.env.CARGO) {
    candidates.push(process.env.CARGO);
  }
  if (process.env.USERPROFILE) {
    candidates.push(join(process.env.USERPROFILE, ".cargo", "bin", exe));
  }
  if (process.env.HOME) {
    candidates.push(join(process.env.HOME, ".cargo", "bin", exe));
  }
  candidates.push("cargo");

  return candidates;
}

function resolveCargo() {
  return cargoCandidates().find((candidate) => {
    return candidate === "cargo" || existsSync(candidate);
  });
}

const cargo = resolveCargo();
const result = spawnSync(cargo, process.argv.slice(2), {
  cwd: process.cwd(),
  stdio: "inherit",
  shell: process.platform === "win32"
});

process.exit(result.status ?? 1);
