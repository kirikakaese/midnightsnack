// SPDX-License-Identifier: GPL-3.0-or-later
// Starts the headless development server with a generated demo show.
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const root = join(import.meta.dirname, "..", "..", "..");

export default async function globalSetup(): Promise<() => void> {
  const exe = join(
    root,
    "target",
    "debug",
    process.platform === "win32" ? "midnightsnack-devserver.exe" : "midnightsnack-devserver",
  );
  if (!existsSync(exe)) {
    throw new Error(
      `${exe} missing: run "cargo build -p midnightsnack-server --bin midnightsnack-devserver"`,
    );
  }
  const dir = mkdtempSync(join(tmpdir(), "msnack-e2e-"));
  const infoFile = join(dir, "info.json");
  const port = process.env.E2E_PORT ?? "47470";
  const child = spawn(exe, ["--demo", "--port", port, "--info-file", infoFile], {
    stdio: ["ignore", "ignore", "inherit"],
    env: { ...process.env, RUST_LOG: "warn" },
  });

  const deadline = Date.now() + 30_000;
  while (!existsSync(infoFile)) {
    if (Date.now() > deadline) throw new Error("devserver did not start");
    if (child.exitCode !== null) throw new Error(`devserver exited with ${child.exitCode}`);
    await new Promise((r) => setTimeout(r, 100));
  }
  process.env.E2E_INFO = readFileSync(infoFile, "utf8");
  return () => child.kill();
}
