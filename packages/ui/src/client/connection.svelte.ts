// SPDX-License-Identifier: GPL-3.0-or-later
// Reactive WebSocket connection to a midnightsnack host, shared by the operator view, output
// windows and the web remote. State is owned by the host; this class only mirrors it.
import {
  PROTOCOL_VERSION,
  type Action,
  type ClientMessage,
  type DeviceInfo,
  type ErrorCode,
  type HostInfo,
  type LiveState,
  type PairingInfo,
  type PendingPairing,
  type Position,
  type ServerMessage,
  type SessionInfo,
  type ShowSnapshot,
} from "@midnightsnack/protocol";

export type ConnectionStatus =
  | "connecting"
  | "connected"
  | "disconnected"
  /** Token rejected: the device must pair again. */
  | "unauthorized"
  /** Host speaks a different protocol version. */
  | "incompatible";

export interface ConnectionOptions {
  /** WebSocket URL, e.g. `ws://192.168.1.5:4747/api/v1/ws`. */
  wsUrl: string;
  /** HTTP base for media, e.g. `http://192.168.1.5:4747`. */
  httpBase: string;
  token: string;
}

const ACTION_TIMEOUT_MS = 5000;
const PING_INTERVAL_MS = 5000;

export class HostConnection {
  status = $state<ConnectionStatus>("connecting");
  host = $state<HostInfo | null>(null);
  session = $state<SessionInfo | null>(null);
  show = $state<ShowSnapshot | null>(null);
  live = $state<LiveState | null>(null);
  devices = $state<DeviceInfo[]>([]);
  pending = $state<PendingPairing[]>([]);
  pairing = $state<PairingInfo | null>(null);
  renderQueued = $state(0);
  /** Round-trip time of the last ping, in ms. */
  latency = $state<number | null>(null);
  /** Last error reported for an action (cleared after a few seconds). */
  lastError = $state<ErrorCode | null>(null);
  /** `host clock - local clock`, to display host timers correctly. */
  clockOffset = 0;

  #opts: ConnectionOptions;
  #ws: WebSocket | null = null;
  #nextId = 1;
  // Internal bookkeeping, intentionally not reactive.
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  #pendingActions = new Map<number, (e: ErrorCode | null) => void>();
  #retryMs = 500;
  #retryTimer: ReturnType<typeof setTimeout> | undefined;
  #pingTimer: ReturnType<typeof setInterval> | undefined;
  #errorTimer: ReturnType<typeof setTimeout> | undefined;
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  #pingSent = new Map<number, number>();
  #closed = false;
  #viewport: { width: number; height: number } | null = null;

  constructor(opts: ConnectionOptions) {
    this.#opts = opts;
  }

  connect(): void {
    this.#closed = false;
    clearTimeout(this.#retryTimer);
    this.status = "connecting";
    let ws: WebSocket;
    try {
      ws = new WebSocket(this.#opts.wsUrl);
    } catch {
      this.#scheduleReconnect();
      return;
    }
    this.#ws = ws;
    ws.onopen = () => {
      this.#send({ type: "hello", protocol_version: PROTOCOL_VERSION, token: this.#opts.token });
    };
    ws.onmessage = (ev) => {
      try {
        this.#handle(JSON.parse(String(ev.data)) as ServerMessage);
      } catch (e) {
        console.error("bad message from host", e);
      }
    };
    ws.onclose = () => {
      if (this.#ws !== ws) return;
      this.#ws = null;
      clearInterval(this.#pingTimer);
      for (const resolve of this.#pendingActions.values()) resolve("internal");
      this.#pendingActions.clear();
      if (this.status === "unauthorized" || this.status === "incompatible" || this.#closed) return;
      this.status = "disconnected";
      this.#scheduleReconnect();
    };
  }

  close(): void {
    this.#closed = true;
    clearTimeout(this.#retryTimer);
    clearInterval(this.#pingTimer);
    this.#ws?.close();
    this.#ws = null;
  }

  #scheduleReconnect(): void {
    if (this.#closed) return;
    clearTimeout(this.#retryTimer);
    this.#retryTimer = setTimeout(() => this.connect(), this.#retryMs);
    this.#retryMs = Math.min(this.#retryMs * 2, 5000);
  }

  #send(msg: ClientMessage): boolean {
    if (this.#ws?.readyState !== WebSocket.OPEN) return false;
    this.#ws.send(JSON.stringify(msg));
    return true;
  }

  #handle(msg: ServerMessage): void {
    switch (msg.type) {
      case "welcome":
        this.host = msg.host;
        this.session = msg.session;
        this.status = "connected";
        this.#retryMs = 500;
        clearInterval(this.#pingTimer);
        this.#pingTimer = setInterval(() => this.#ping(), PING_INTERVAL_MS);
        this.#ping();
        if (this.#viewport) this.reportViewport(this.#viewport.width, this.#viewport.height);
        break;
      case "session":
        this.session = msg.session;
        break;
      case "show":
        this.show = msg.show;
        break;
      case "live":
        this.live = msg.live;
        this.clockOffset = msg.live.host_time_ms - Date.now();
        break;
      case "devices":
        this.devices = msg.devices;
        this.pending = msg.pending;
        break;
      case "pairing":
        this.pairing = msg.pairing;
        break;
      case "render_progress":
        this.renderQueued = msg.queued;
        break;
      case "action_result": {
        const resolve = this.#pendingActions.get(msg.request_id);
        this.#pendingActions.delete(msg.request_id);
        resolve?.(msg.error);
        if (msg.error) this.#flashError(msg.error);
        break;
      }
      case "pong": {
        const sent = this.#pingSent.get(msg.nonce);
        this.#pingSent.delete(msg.nonce);
        if (sent !== undefined) this.latency = Math.round(performance.now() - sent);
        break;
      }
      case "error":
        if (msg.code === "unauthorized") this.status = "unauthorized";
        else if (msg.code === "protocol_mismatch") this.status = "incompatible";
        else this.#flashError(msg.code);
        break;
    }
  }

  #ping(): void {
    const nonce = this.#nextId++;
    this.#pingSent.set(nonce, performance.now());
    if (this.#pingSent.size > 10) this.#pingSent.delete(this.#pingSent.keys().next().value!);
    this.#send({ type: "ping", nonce });
  }

  #flashError(code: ErrorCode): void {
    this.lastError = code;
    clearTimeout(this.#errorTimer);
    this.#errorTimer = setTimeout(() => (this.lastError = null), 4000);
  }

  /** Sends an action. Resolves with `null` on success or the error code. */
  action(action: Action): Promise<ErrorCode | null> {
    const request_id = this.#nextId++;
    if (!this.#send({ type: "action", request_id, action })) {
      return Promise.resolve("internal");
    }
    return new Promise((resolve) => {
      this.#pendingActions.set(request_id, resolve);
      setTimeout(() => {
        if (this.#pendingActions.delete(request_id)) resolve("internal");
      }, ACTION_TIMEOUT_MS);
    });
  }

  /** Output windows report their size so slides render at native resolution. */
  reportViewport(width: number, height: number): void {
    this.#viewport = { width, height };
    this.#send({ type: "viewport", width, height });
  }

  /** URL of a slide image, or `null` for positions without an image (blank cues). */
  slideUrl(pos: Position | null | undefined, width?: number, height?: number): string | null {
    if (!pos || !this.session || !this.show) return null;
    const cue = this.show.cues.find((c) => c.id === pos.cue_id);
    if (!cue || cue.kind === "blank" || pos.slide >= cue.slide_count) return null;
    const size = width && height ? `&w=${Math.round(width)}&h=${Math.round(height)}` : "";
    return (
      `${this.#opts.httpBase}/api/v1/media/slide/${encodeURIComponent(pos.cue_id)}/${pos.slide}` +
      `?k=${this.session.media_key}${size}`
    );
  }

  /** URL of a video/audio cue's file (supports range requests). */
  mediaUrl(cueId: string): string | null {
    if (!this.session) return null;
    return `${this.#opts.httpBase}/api/v1/media/file/${encodeURIComponent(cueId)}?k=${this.session.media_key}`;
  }

  /** URL of an image asset (logo, background, logo bug). */
  assetUrl(assetId: string | null | undefined): string | null {
    if (!assetId || !this.session) return null;
    return `${this.#opts.httpBase}/api/v1/media/asset/${encodeURIComponent(assetId)}?k=${this.session.media_key}`;
  }

  cue(id: string | undefined | null) {
    return id ? (this.show?.cues.find((c) => c.id === id) ?? null) : null;
  }

  /** Host time now, in epoch ms. */
  hostNow(): number {
    return Date.now() + this.clockOffset;
  }
}
