const assert = require("node:assert/strict");
const { test } = require("bun:test");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");

const { resolveBinaryCandidates } = require("./agent-kernel-lib");

test("prefers packaged platform binary before local cargo targets", () => {
  const root = "C:\\repo\\agent-kernel";
  const candidates = resolveBinaryCandidates({
    root,
    platform: "win32",
    arch: "x64"
  });

  assert.equal(candidates[0], join(root, "bin", "win32-x64", "agent-kernel.exe"));
  assert.equal(candidates[1], join(root, "target", "release", "agent-kernel.exe"));
  assert.equal(candidates[2], join(root, "target", "debug", "agent-kernel.exe"));
});

test("uses non-windows executable names for packaged binaries", () => {
  const root = "/repo/agent-kernel";
  const candidates = resolveBinaryCandidates({
    root,
    platform: "linux",
    arch: "arm64"
  });

  assert.equal(candidates[0], join(root, "bin", "linux-arm64", "agent-kernel"));
});

test("package metadata prefers Bun wrapper and scripts", () => {
  const pkg = JSON.parse(readFileSync(join(__dirname, "..", "package.json"), "utf8"));

  assert.equal(pkg.bin["agent-kernel"], "bun/agent-kernel.js");
  assert.equal(pkg.scripts.build, "bun bun/cargo.js build");
  assert.match(pkg.scripts.test, /^bun test bun\/agent-kernel-lib\.test\.js/);
  assert.equal(pkg.scripts.start, "bun bun/cargo.js run --");
});
