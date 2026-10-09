// SPDX-License-Identifier: GPL-3.0-or-later
// Runs the client against the real development server when it has been built
// (`cargo build -p midnightsnack-server --bin midnightsnack-devserver`).
import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import WebSocket from "ws";
import { HostClient } from "../src/client";
import { variables } from "../src/state";

const exe = join(import.meta.dirname, "../../../target/debug/midnightsnack-devserver");
const available = existsSync(exe);

let server: ChildProcess | undefined;
let info: { port: number; operator_token: string };

async function until(cond: () => boolean, ms = 10_000): Promise<void> {
  const end = Date.now() + ms;
  while (!cond()) {
    if (Date.now() > end) throw new Error("timeout");
    await new Promise((r) => setTimeout(r, 20));
  }
}

describe.skipIf(!available)("against the development server", () => {
  beforeAll(async () => {
    const dir = mkdtempSync(join(tmpdir(), "msnack-companion-"));
    const file = join(dir, "info.json");
    server = spawn(exe, ["--demo", "--port", "0", "--info-file", file], {
      stdio: "ignore",
      env: { ...process.env, RUST_LOG: "warn" },
    });
    await until(() => existsSync(file), 30_000);
    info = JSON.parse(readFileSync(file, "utf8"));
  }, 40_000);

  afterAll(() => server?.kill());

  /** Creates an operator API key over the admin connection, like the Control tab does. */
  async function apiKey(): Promise<string> {
    const ws = new WebSocket(`ws://127.0.0.1:${info.port}/api/v1/ws`);
    await new Promise((r) => ws.once("open", r));
    ws.send(JSON.stringify({ type: "hello", protocol_version: 1, token: info.operator_token }));
    ws.send(JSON.stringify({ type: "create_api_key", name: "Companion", role: "operator" }));
    const token = await new Promise<string>((resolve) =>
      ws.on("message", (d) => {
        const m = JSON.parse(String(d));
        if (m.type === "api_key") resolve(m.token);
      }),
    );
    ws.close();
    return token;
  }

  it("connects, runs actions and mirrors state", async () => {
    const statuses: string[] = [];
    const key = await apiKey();
    const client = new HostClient(`ws://127.0.0.1:${info.port}/api/v1/ws`, key, {
      status: (s) => statuses.push(s),
      state: () => {},
    });
    client.connect();
    await until(() => client.status === "ok" && !!client.show && !!client.live);
    expect(
      await client.action({
        action: "go_to",
        position: { cue_id: client.show!.cues[0]!.id, slide: 0 },
      }),
    ).toBeNull();
    expect(await client.action({ action: "set_blackout", on: true })).toBeNull();
    await until(() => !!client.live?.masters.blackout);
    const v = variables(client.show, client.live, client.hostNow());
    expect(v.cue_number).toBe(1);
    expect(v.slide).toBe(1);
    expect(await client.action({ action: "set_blackout", on: false })).toBeNull();
    // An operator key cannot edit the show.
    expect(await client.action({ action: "new_show" })).toBe("forbidden");
    client.close();
  });

  it("reports a rejected key", async () => {
    const client = new HostClient(`ws://127.0.0.1:${info.port}/api/v1/ws`, "wrong", {
      status: () => {},
      state: () => {},
    });
    client.connect();
    await until(() => client.status === "unauthorized");
    client.close();
  });
});
