const assert = require("node:assert/strict");
const { test } = require("node:test");
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
