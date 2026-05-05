const assert = require("node:assert/strict");
const { test } = require("bun:test");
const { readFileSync } = require("node:fs");
const { join } = require("node:path");

const {
  isSupportedPackagedTarget,
  resolveBinaryCandidates,
  supportedPackagedTargets
} = require("./agent-kernel-lib");

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
    arch: "x64"
  });

  assert.equal(candidates[0], join(root, "bin", "linux-x64", "agent-kernel"));
});

test("unsupported packaged targets still fall back to cargo outputs", () => {
  const root = "/repo/agent-kernel";
  const candidates = resolveBinaryCandidates({
    root,
    platform: "linux",
    arch: "arm64"
  });

  assert.equal(candidates[0], join(root, "target", "release", "agent-kernel"));
  assert.equal(candidates[1], join(root, "target", "debug", "agent-kernel"));
});

test("declares packaged targets for windows macos and linux", () => {
  assert.deepEqual(supportedPackagedTargets(), [
    "win32-x64",
    "linux-x64",
    "darwin-x64",
    "darwin-arm64"
  ]);

  assert.equal(isSupportedPackagedTarget("win32", "x64"), true);
  assert.equal(isSupportedPackagedTarget("linux", "x64"), true);
  assert.equal(isSupportedPackagedTarget("darwin", "x64"), true);
  assert.equal(isSupportedPackagedTarget("darwin", "arm64"), true);
  assert.equal(isSupportedPackagedTarget("freebsd", "x64"), false);
});

test("package metadata prefers Bun wrapper and scripts", () => {
  const pkg = JSON.parse(readFileSync(join(__dirname, "..", "package.json"), "utf8"));

  assert.equal(pkg.bin["agent-kernel"], "bun/agent-kernel.js");
  assert.equal(pkg.scripts.build, "bun bun/cargo.js build");
  assert.match(pkg.scripts.test, /^bun test bun\/agent-kernel-lib\.test\.js/);
  assert.match(pkg.scripts.test, /bun run --cwd app test/);
  assert.equal(pkg.scripts["app:test"], "bun run --cwd app test");
  assert.equal(pkg.scripts.start, "bun bun/cargo.js run --");
});

test("ci workflow verifies windows macos and linux", () => {
  const ci = readFileSync(join(__dirname, "..", ".github", "workflows", "ci.yml"), "utf8");

  assert.match(ci, /windows-latest/);
  assert.match(ci, /ubuntu-latest/);
  assert.match(ci, /macos-latest/);
  assert.match(ci, /Install Linux Tauri dependencies/);
});

test("release workflow packages the supported targets", () => {
  const release = readFileSync(
    join(__dirname, "..", ".github", "workflows", "release.yml"),
    "utf8"
  );

  for (const target of supportedPackagedTargets()) {
    assert.match(release, new RegExp(target));
  }
  assert.match(release, /Install Linux Tauri dependencies/);
});
