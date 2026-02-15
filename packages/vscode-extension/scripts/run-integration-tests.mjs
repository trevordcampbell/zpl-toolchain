import { spawnSync } from "node:child_process";

const testEntry = "./dist/test/runTest.js";

function run(command, args) {
  const result = spawnSync(command, args, {
    stdio: "inherit",
    shell: false,
    env: process.env,
  });
  if (typeof result.status === "number") {
    process.exit(result.status);
  }
  process.exit(1);
}

if (process.platform === "linux") {
  const check = spawnSync("xvfb-run", ["--help"], {
    stdio: "ignore",
    shell: false,
  });
  if (check.error) {
    console.error(
      "xvfb-run is required for Linux extension integration tests. Install xvfb first."
    );
    process.exit(1);
  }
  run("xvfb-run", ["-a", "node", testEntry]);
} else {
  run("node", [testEntry]);
}
