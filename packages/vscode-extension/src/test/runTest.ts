import * as path from "node:path";
import { spawnSync } from "node:child_process";

import { runTests } from "@vscode/test-electron";

const ARM64_EXECUTABLE_CANDIDATES = ["code", "code-insiders", "cursor", "codium"];

function looksLikeEditorVersionOutput(output: string): boolean {
  const firstLine = output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => line.length > 0);
  if (!firstLine) {
    return false;
  }
  // VS Code-family CLIs report versions like "1.109.3".
  // Node runtimes report versions like "v22.21.1", which are not usable here.
  return /^\d+\.\d+\.\d+/.test(firstLine);
}

function supportsExtensionHostFlags(executable: string): boolean {
  const help = spawnSync(executable, ["--help"], {
    shell: false,
    encoding: "utf8",
  });
  if (help.error || help.status !== 0) {
    return false;
  }
  const helpText = `${help.stdout ?? ""}\n${help.stderr ?? ""}`;
  return (
    helpText.includes("--extensionDevelopmentPath") &&
    helpText.includes("--extensionTestsPath")
  );
}

function resolveArm64ExecutablePath(): string | undefined {
  for (const candidate of ARM64_EXECUTABLE_CANDIDATES) {
    const result = spawnSync(candidate, ["--version"], {
      shell: false,
      encoding: "utf8",
    });
    if (result.error || result.status !== 0) {
      continue;
    }
    const combinedOutput = `${result.stdout ?? ""}\n${result.stderr ?? ""}`.trim();
    if (looksLikeEditorVersionOutput(combinedOutput) && supportsExtensionHostFlags(candidate)) {
      return candidate;
    }
  }
  return undefined;
}

async function main(): Promise<void> {
  const isLinuxArm64 = process.platform === "linux" && process.arch === "arm64";
  const forced = process.env.FORCE_VSCODE_INTEGRATION === "1";
  const configuredExecutable = process.env.VSCODE_EXECUTABLE_PATH?.trim();
  const discoveredExecutable =
    !configuredExecutable && isLinuxArm64 ? resolveArm64ExecutablePath() : undefined;
  const overrideExecutable = configuredExecutable || discoveredExecutable;

  if (discoveredExecutable) {
    console.log(
      `linux/arm64: using discovered editor executable for integration tests: ${discoveredExecutable}`
    );
  }

  if (configuredExecutable && isLinuxArm64 && !supportsExtensionHostFlags(configuredExecutable)) {
    throw new Error(
      `Configured VSCODE_EXECUTABLE_PATH ('${configuredExecutable}') does not expose ` +
        "Extension Host test flags (--extensionDevelopmentPath/--extensionTestsPath)."
    );
  }

  if (isLinuxArm64 && !forced && !overrideExecutable) {
    console.warn(
      "Skipping Extension Host integration tests on linux/arm64. " +
        "Set VSCODE_EXECUTABLE_PATH (or install a supported editor CLI such as code/cursor/codium), " +
        "or set FORCE_VSCODE_INTEGRATION=1 to force execution."
    );
    return;
  }

  const extensionDevelopmentPath = path.resolve(__dirname, "../..");
  const extensionTestsPath = path.resolve(__dirname, "./suite/index");
  process.env.ZPL_VSCODE_EXTENSION_ROOT = extensionDevelopmentPath;
  process.env.ZPL_REPO_ROOT = path.resolve(extensionDevelopmentPath, "../..");

  await runTests({
    extensionDevelopmentPath,
    extensionTestsPath,
    launchArgs: ["--disable-extensions"],
    ...(overrideExecutable ? { vscodeExecutablePath: overrideExecutable } : {}),
  });
}

main().catch((error) => {
  console.error("Failed to run VS Code extension integration tests.");
  console.error(error);
  process.exit(1);
});
