// SPDX-License-Identifier: GPL-3.0-or-later
// WebSocket client for a midnightsnack host: authenticates with an API key, mirrors the show and
// live state, sends actions and reconnects on its own.
import type {
  Action,
  ClientMessage,
  ErrorCode,
  LiveState,
  ServerMessage,
  ShowSnapshot,
} from "@midnightsnack/protocol";
import WebSocket from "ws";

/** Must match `PROTOCOL_VERSION` in `crates/protocol` (checked by a test). */
export const PROTOCOL_VERSION = 1;

export type ClientStatus = "connecting" | "ok" | "disconnected" | "unauthorized" | "incompatible";

export interface ClientEvents {
  status: (status: ClientStatus) => void;
  /** Show or live state changed. */
  state: () => void;
}

const ACTION_TIMEOUT_MS = 5000;

export class HostClient {
  show: ShowSnapshot | null = null;
  live: LiveState | null = null;
  status: ClientStatus = "connecting";
  /** Host clock minus local clock. */
  clockOffset = 0;

  #url: string;
  #token: string;
  #events: ClientEvents;
  #ws: WebSocket | null = null;
  #nextId = 1;
  #pending = new Map<number, (e: ErrorCode | null) => void>();
  #retryMs = 500;
  #retry: ReturnType<typeof setTimeout> | undefined;
  #closed = false;

  constructor(url: string, token: string, events: ClientEvents) {
    this.#url = url;
    this.#token = token;
    this.#events = events;
  }

  connect(): void {
    this.#closed = false;
    clearTimeout(this.#retry);
    this.#setStatus("connecting");
    const ws = new WebSocket(this.#url);
    this.#ws = ws;
    ws.on("open", () => {
      this.#send({ type: "hello", protocol_version: PROTOCOL_VERSION, token: this.#token });
    });
    ws.on("message", (data) => {
      try {
        this.#handle(JSON.parse(String(data)) as ServerMessage);
      } catch {
        // Ignore malformed messages.
      }
    });
    ws.on("error", () => {
      // `close` follows and schedules the reconnect.
    });
    ws.on("close", () => {
      if (this.#ws !== ws) return;
      this.#ws = null;
      for (const resolve of this.#pending.values()) resolve("internal");
      this.#pending.clear();
      if (this.#closed || this.status === "unauthorized" || this.status === "incompatible") return;
      this.#setStatus("disconnected");
      this.#retry = setTimeout(() => this.connect(), this.#retryMs);
      this.#retryMs = Math.min(this.#retryMs * 2, 5000);
    });
  }

  close(): void {
    this.#closed = true;
    clearTimeout(this.#retry);
    this.#ws?.close();
    this.#ws = null;
  }

  /** Sends an action; resolves with `null` on success or the error code. */
  action(action: Action): Promise<ErrorCode | null> {
    const request_id = this.#nextId++;
    if (!this.#send({ type: "action", request_id, action })) return Promise.resolve("internal");
    return new Promise((resolve) => {
      this.#pending.set(request_id, resolve);
      setTimeout(() => {
        if (this.#pending.delete(request_id)) resolve("internal");
      }, ACTION_TIMEOUT_MS);
    });
  }

  /** Host time now (epoch ms). */
  hostNow(): number {
    return Date.now() + this.clockOffset;
  }

  #send(msg: ClientMessage): boolean {
    if (this.#ws?.readyState !== WebSocket.OPEN) return false;
    this.#ws.send(JSON.stringify(msg));
    return true;
  }

  #setStatus(status: ClientStatus): void {
    if (this.status === status) return;
    this.status = status;
    this.#events.status(status);
  }

  #handle(msg: ServerMessage): void {
    switch (msg.type) {
      case "welcome":
        this.#retryMs = 500;
        this.#setStatus("ok");
        break;
      case "show":
        this.show = msg.show;
        this.#events.state();
        break;
      case "live":
        this.live = msg.live;
        this.clockOffset = msg.live.host_time_ms - Date.now();
        this.#events.state();
        break;
      case "action_result": {
        const resolve = this.#pending.get(msg.request_id);
        this.#pending.delete(msg.request_id);
        resolve?.(msg.error);
        break;
      }
      case "error":
        if (msg.code === "unauthorized") this.#setStatus("unauthorized");
        else if (msg.code === "protocol_mismatch") this.#setStatus("incompatible");
        break;
      default:
        break;
    }
  }
}
