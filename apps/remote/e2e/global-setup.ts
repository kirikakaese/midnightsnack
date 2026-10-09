// SPDX-License-Identifier: GPL-3.0-or-later
// Starts the headless development server with a generated demo show.
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const root = join(import.meta.dirname, "..", "..", "..");

function binary(name: string, pkg: string): string {
  const exe = join(root, "target", "debug", process.platform === "win32" ? `${name}.exe` : name);
  if (!existsSync(exe)) {
    throw new Error(`${exe} missing: run "cargo build -p ${pkg} --bin ${name}"`);
  }
  return exe;
}

export default async function globalSetup(): Promise<() => void> {
  const dir = mkdtempSync(join(tmpdir(), "msnack-e2e-"));
  const infoFile = join(dir, "info.json");
  const port = process.env.E2E_PORT ?? "47470";
  const relayPort = process.env.E2E_RELAY_PORT ?? "47480";
  // A relay on this machine stands in for one on the internet.
  const relay = spawn(
    binary("midnightsnack-relay", "midnightsnack-relay"),
    ["--listen", `127.0.0.1:${relayPort}`],
    { stdio: ["ignore", "ignore", "inherit"], env: { ...process.env, RUST_LOG: "warn" } },
  );
  const child = spawn(
    binary("midnightsnack-devserver", "midnightsnack-server"),
    ["--demo", "--port", port, "--info-file", infoFile, "--relay", `http://127.0.0.1:${relayPort}`],
    {
      stdio: ["ignore", "ignore", "inherit"],
      env: { ...process.env, RUST_LOG: "warn" },
    },
  );

  const deadline = Date.now() + 30_000;
  while (!existsSync(infoFile)) {
    if (Date.now() > deadline) throw new Error("devserver did not start");
    if (child.exitCode !== null) throw new Error(`devserver exited with ${child.exitCode}`);
    await new Promise((r) => setTimeout(r, 100));
  }
  const info = JSON.parse(readFileSync(infoFile, "utf8")) as Record<string, unknown>;
  process.env.E2E_INFO = JSON.stringify({ ...info, relay_port: Number(relayPort) });
  return () => {
    child.kill();
    relay.kill();
  };
}
