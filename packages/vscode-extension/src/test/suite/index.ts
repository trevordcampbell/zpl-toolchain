import * as fs from "node:fs";
import * as path from "node:path";

import Mocha from "mocha";

function collectTestFiles(dir: string, out: string[]): void {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.resolve(dir, entry.name);
    if (entry.isDirectory()) {
      collectTestFiles(fullPath, out);
      continue;
    }
    if (entry.isFile() && entry.name.endsWith(".test.js")) {
      out.push(fullPath);
    }
  }
}

export function run(): Promise<void> {
  const mocha = new Mocha({
    ui: "tdd",
    color: true,
    timeout: 60_000,
  });

  const testsRoot = __dirname;
  const files: string[] = [];
  collectTestFiles(testsRoot, files);
  for (const file of files) {
    mocha.addFile(file);
  }

  return new Promise((resolve, reject) => {
    mocha.run((failures) => {
      if (failures > 0) {
        reject(new Error(`${failures} integration test(s) failed.`));
      } else {
        resolve();
      }
    });
  });
}
