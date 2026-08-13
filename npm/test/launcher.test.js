import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
const launcher = join(here, "..", "bin", "whyhot.js");

test("launcher runs the bundled binary", { skip: process.platform !== "darwin" }, () => {
  const output = execFileSync(process.execPath, [launcher, "--version"], {
    encoding: "utf8"
  });
  assert.equal(output.trim(), "whyhot 0.1.0");
});
