// SPDX-License-Identifier: GPL-3.0-or-later
// Load test against the development server: N paired phones (some moving a laser pointer at
// 30 Hz), an operator sending next/previous, and the time until every connection has the new
// live state. Reports latency percentiles and the server's memory over the run.
//
//   cargo build --release -p midnightsnack-server --bin midnightsnack-devserver
//   node scripts/bench.mjs [--remotes 25] [--pointers 5] [--seconds 60] [--debug]
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? Number(args[i + 1]) : def;
};
const REMOTES = opt("remotes", 25);
const POINTERS = opt("pointers", 5);
const SECONDS = opt("seconds", 60);
const profile = args.includes("--debug") ? "debug" : "release";
const root = join(import.meta.dirname, "..");
const exe = join(root, "target", profile, "midnightsnack-devserver");
if (!existsSync(exe)) {
  console.error(`${exe} missing; build it first (see the header of this script)`);
  process.exit(2);
}

const dir = mkdtempSync(join(tmpdir(), "msnack-bench-"));
const infoFile = join(dir, "info.json");
const port = 47900 + Math.floor(Math.random() * 90);
const server = spawn(exe, ["--demo", "--port", String(port), "--info-file", infoFile], {
  stdio: ["ignore", "ignore", "inherit"],
  env: { ...process.env, RUST_LOG: "warn" },
});
process.on("exit", () => server.kill());
while (!existsSync(infoFile)) await new Promise((r) => setTimeout(r, 100));
const info = JSON.parse(readFileSync(infoFile, "utf8"));
const base = `http://127.0.0.1:${port}`;
const wsUrl = `ws://127.0.0.1:${port}/api/v1/ws`;

function rssMb(pid) {
  const status = readFileSync(`/proc/${pid}/status`, "utf8");
  return Number(/VmRSS:\s+(\d+)/.exec(status)[1]) / 1024;
}

/** A WebSocket client that records when each live revision arrives. */
function client(token) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    const c = { ws, live: null, seen: new Map(), pairing: null, id: 0, waiters: [] };
    ws.onopen = () => ws.send(JSON.stringify({ type: "hello", protocol_version: 1, token }));
    ws.onerror = reject;
    ws.onmessage = (ev) => {
      const m = JSON.parse(ev.data);
      if (m.type === "live") {
        c.live = m.live;
        c.seen.set(m.live.revision, performance.now());
      }
      if (m.type === "pairing") c.pairing = m.pairing;
      if (m.type === "welcome") resolve(c);
      for (const w of c.waiters) w(m);
    };
  });
}

function action(c, a) {
  return new Promise((resolve) => {
    const request_id = ++c.id;
    const w = (m) => {
      if (m.type === "action_result" && m.request_id === request_id) {
        c.waiters = c.waiters.filter((x) => x !== w);
        resolve(m.error);
      }
    };
    c.waiters.push(w);
    c.ws.send(JSON.stringify({ type: "action", request_id, action: a }));
  });
}

const op = await client(info.operator_token);
await action(op, { action: "set_auto_approve", role: "operator" });
await new Promise((r) => setTimeout(r, 200));

async function pairOne(i) {
  const join = op.pairing.links[0].url.split("#t=")[1];
  const res = await fetch(`${base}/api/v1/pair`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ join_token: join, pin: op.pairing.pin, device_name: `Bench ${i}` }),
  });
  const { request_id } = await res.json();
  // The join token rotates after each use; wait for the new one.
  const before = op.pairing.links[0].url;
  for (let k = 0; k < 50 && op.pairing.links[0].url === before; k++) {
    await new Promise((r) => setTimeout(r, 10));
  }
  const status = await (await fetch(`${base}/api/v1/pair/${request_id}`)).json();
  return status.token;
}

const remotes = [];
for (let i = 0; i < REMOTES; i++) remotes.push(await client(await pairOne(i)));
console.log(`${REMOTES} remotes connected, ${POINTERS} moving pointers`);

const pointerTimers = remotes.slice(0, POINTERS).map((r, i) =>
  setInterval(() => {
    const t = performance.now() / 1000;
    const pos = [0.5 + 0.4 * Math.cos(t + i), 0.5 + 0.4 * Math.sin(t + i)];
    r.ws.send(JSON.stringify({ type: "pointer", pos, mode: "point", color: "#ff3b30" }));
  }, 33),
);

await action(op, { action: "go" });
const samples = [];
const memory = [rssMb(server.pid)];
const end = performance.now() + SECONDS * 1000;
let n = 0;
while (performance.now() < end) {
  const t0 = performance.now();
  const before = op.live.revision;
  await action(op, n++ % 2 ? { action: "prev" } : { action: "next" });
  // Wait until every connection saw a newer revision.
  const all = [op, ...remotes];
  const deadline = t0 + 2000;
  while (performance.now() < deadline && !all.every((c) => c.live.revision > before)) {
    await new Promise((r) => setTimeout(r, 0));
  }
  const last = Math.max(...all.map((c) => c.seen.get(c.live.revision) ?? Infinity));
  samples.push(last - t0);
  if (n % 50 === 0) memory.push(rssMb(server.pid));
  await new Promise((r) => setTimeout(r, 40));
}
memory.push(rssMb(server.pid));
pointerTimers.forEach(clearInterval);

samples.sort((a, b) => a - b);
const pct = (p) => samples[Math.min(samples.length - 1, Math.floor((p / 100) * samples.length))];
const result = {
  build: profile,
  remotes: REMOTES,
  pointers: POINTERS,
  actions: samples.length,
  latency_ms: {
    p50: +pct(50).toFixed(2),
    p95: +pct(95).toFixed(2),
    p99: +pct(99).toFixed(2),
    max: +samples[samples.length - 1].toFixed(2),
  },
  server_rss_mb: {
    start: +memory[0].toFixed(1),
    peak: +Math.max(...memory).toFixed(1),
    end: +memory[memory.length - 1].toFixed(1),
  },
};
console.log(JSON.stringify(result, null, 2));
for (const c of [op, ...remotes]) c.ws.close();
server.kill();
process.exit(0);
