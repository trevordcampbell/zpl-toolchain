const SUPPORTED_TARGETS = {
  "linux:x64": {
    target: "x86_64-unknown-linux-gnu",
    archiveExt: "tar.gz",
    binaryName: "zpl",
  },
  "darwin:arm64": {
    target: "aarch64-apple-darwin",
    archiveExt: "tar.gz",
    binaryName: "zpl",
  },
  "win32:x64": {
    target: "x86_64-pc-windows-msvc",
    archiveExt: "zip",
    binaryName: "zpl.exe",
  },
};

export function resolveTarget(platform, arch) {
  return SUPPORTED_TARGETS[`${platform}:${arch}`] ?? null;
}

