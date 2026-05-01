const { existsSync } = require("node:fs");
const { join } = require("node:path");

const PACKAGED_TARGETS = new Set([
  "win32-x64",
  "linux-x64",
  "darwin-x64",
  "darwin-arm64"
]);

function executableName(platform) {
  return platform === "win32" ? "agent-kernel.exe" : "agent-kernel";
}

function platformArch(platform, arch) {
  return `${platform}-${arch}`;
}

function supportedPackagedTargets() {
  return Array.from(PACKAGED_TARGETS);
}

function isSupportedPackagedTarget(platform, arch) {
  return PACKAGED_TARGETS.has(platformArch(platform, arch));
}

function resolveBinaryCandidates({
  root,
  platform = process.platform,
  arch = process.arch
}) {
  const exe = executableName(platform);
  const candidates = [
    join(root, "target", "release", exe),
    join(root, "target", "debug", exe)
  ];
  if (isSupportedPackagedTarget(platform, arch)) {
    candidates.unshift(join(root, "bin", platformArch(platform, arch), exe));
  }
  return candidates;
}

function findExistingBinary(options) {
  return resolveBinaryCandidates(options).find((candidate) => existsSync(candidate));
}

module.exports = {
  executableName,
  findExistingBinary,
  isSupportedPackagedTarget,
  platformArch,
  resolveBinaryCandidates,
  supportedPackagedTargets
};
