// SPDX-License-Identifier: GPL-3.0-or-later
// Launches the debug host build with MIDNIGHTSNACK_SMOKE_TEST=1. The app exits with code 0
// once the operator window has loaded and talked to the backend; anything else is a failure.
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";

const exe = join(
  import.meta.dirname,
  "..",
  "target",
  "debug",
  process.platform === "win32" ? "deck.exe" : "deck",
);
if (!existsSync(exe)) {
  console.error(`smoke test: ${exe} not found; build the host first`);
  process.exit(1);
}

const timeoutMs = Number(process.env.SMOKE_TIMEOUT_MS ?? 60000);
const child = spawn(exe, [], {
  stdio: "inherit",
  env: { ...process.env, MIDNIGHTSNACK_SMOKE_TEST: "1" },
});
const timer = setTimeout(() => {
  console.error(`smoke test: no ready signal after ${timeoutMs} ms`);
  child.kill();
  process.exit(1);
}, timeoutMs);
child.on("exit", (code) => {
  clearTimeout(timer);
  console.log(`smoke test: host exited with code ${code}`);
  process.exit(code === 0 ? 0 : 1);
});
