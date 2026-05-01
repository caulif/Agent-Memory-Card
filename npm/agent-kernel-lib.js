const { existsSync } = require("node:fs");
const { join } = require("node:path");

function executableName(platform) {
  return platform === "win32" ? "agent-kernel.exe" : "agent-kernel";
}

function platformArch(platform, arch) {
  return `${platform}-${arch}`;
}

function resolveBinaryCandidates({
  root,
  platform = process.platform,
  arch = process.arch
}) {
  const exe = executableName(platform);
  return [
    join(root, "bin", platformArch(platform, arch), exe),
    join(root, "target", "release", exe),
    join(root, "target", "debug", exe)
  ];
}

function findExistingBinary(options) {
  return resolveBinaryCandidates(options).find((candidate) => existsSync(candidate));
}

module.exports = {
  executableName,
  findExistingBinary,
  platformArch,
  resolveBinaryCandidates
};
