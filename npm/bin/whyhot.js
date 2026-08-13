#!/usr/bin/env node

import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

if (process.platform !== "darwin") {
  console.error("whyhot supports macOS only.");
  process.exit(1);
}

const architectures = {
  arm64: "whyhot-arm64",
  x64: "whyhot-x64"
};
const executable = architectures[process.arch];

if (!executable) {
  console.error(`whyhot does not support the ${process.arch} architecture.`);
  process.exit(1);
}

const here = dirname(fileURLToPath(import.meta.url));
const binary = join(here, "..", "native", executable);

if (!existsSync(binary)) {
  console.error(`whyhot's ${process.arch} binary is missing. Please reinstall the package.`);
  process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });

if (result.error) {
  console.error(`whyhot failed to start: ${result.error.message}`);
  process.exit(1);
}

if (result.signal) {
  process.kill(process.pid, result.signal);
}

process.exit(result.status ?? 1);
