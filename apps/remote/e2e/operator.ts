// SPDX-License-Identifier: GPL-3.0-or-later
// A minimal operator client used by tests to approve devices and inspect live state.
import type {
  Action,
  InboxItem,
  LiveState,
  PendingPairing,
  ServerMessage,
  ShowSnapshot,
} from "@midnightsnack/protocol";

export interface DevInfo {
  port: number;
  relay_port: number;
  operator_token: string;
  pin: string;
  join_url: string;
  relay_join_url: string | null;
}

export function devInfo(): DevInfo {
  return JSON.parse(process.env.E2E_INFO ?? "{}") as DevInfo;
}

export class Operator {
  #ws: WebSocket;
  #listeners: Array<(m: ServerMessage) => void> = [];
  #id = 0;
  live: LiveState | null = null;
  inbox: InboxItem[] = [];
  show: ShowSnapshot | null = null;
  pending: PendingPairing[] = [];
  pin = "";
  /** Last pointer message per device. */
  pointers = new Map<string, Extract<ServerMessage, { type: "pointer" }>>();
  joinUrl = "";
  /** Join link through the relay (while the host is connected to it). */
  relayJoinUrl = "";

  private constructor(ws: WebSocket) {
    this.#ws = ws;
    ws.onmessage = (ev) => {
      const m = JSON.parse(String(ev.data)) as ServerMessage;
      if (m.type === "live") this.live = m.live;
      if (m.type === "show") this.show = m.show;
      if (m.type === "inbox") this.inbox = m.items;
      if (m.type === "pointer") this.pointers.set(m.device_id, m);
      if (m.type === "devices") this.pending = m.pending;
      if (m.type === "pairing") {
        this.pin = m.pairing.pin;
        this.joinUrl = m.pairing.links.find((l) => l.kind === "lan")?.url ?? "";
        this.relayJoinUrl = m.pairing.links.find((l) => l.kind === "relay")?.url ?? "";
      }
      for (const l of this.#listeners) l(m);
    };
  }

  static async connect(info: DevInfo): Promise<Operator> {
    const ws = new WebSocket(`ws://127.0.0.1:${info.port}/api/v1/ws`);
    await new Promise((resolve, reject) => {
      ws.onopen = resolve;
      ws.onerror = reject;
    });
    const op = new Operator(ws);
    ws.send(JSON.stringify({ type: "hello", protocol_version: 1, token: info.operator_token }));
    await op.waitFor((m) => m.type === "pairing");
    return op;
  }

  waitFor(pred: (m: ServerMessage) => boolean, timeoutMs = 10_000): Promise<ServerMessage> {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("timeout waiting for message")), timeoutMs);
      const l = (m: ServerMessage) => {
        if (pred(m)) {
          clearTimeout(timer);
          this.#listeners = this.#listeners.filter((x) => x !== l);
          resolve(m);
        }
      };
      this.#listeners.push(l);
    });
  }

  async action(action: Action): Promise<void> {
    const request_id = ++this.#id;
    this.#ws.send(JSON.stringify({ type: "action", request_id, action }));
    const res = await this.waitFor(
      (m) => m.type === "action_result" && m.request_id === request_id,
    );
    if (res.type === "action_result" && res.error) throw new Error(res.error);
  }

  async nextPending(): Promise<PendingPairing> {
    if (this.pending.length) return this.pending[0]!;
    const m = await this.waitFor((m) => m.type === "devices" && m.pending.length > 0);
    if (m.type !== "devices") throw new Error("unreachable");
    return m.pending[0]!;
  }

  close(): void {
    this.#ws.close();
  }
}
