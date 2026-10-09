// SPDX-License-Identifier: GPL-3.0-or-later
// Reactive WebSocket connection to a midnightsnack host, shared by the operator view, output
// windows and the web remote. State is owned by the host; this class only mirrors it.
import {
  PROTOCOL_VERSION,
  type Action,
  type CaptureTarget,
  type ClientMessage,
  type ConnectivityInfo,
  type ControlSettings,
  type DeviceInfo,
  type ErrorCode,
  type HostInfo,
  type InboxItem,
  type OpenSlidesStatus,
  type OsMeetingData,
  type LiveState,
  type PairingInfo,
  type PendingPairing,
  type PointerMode,
  type Position,
  type Role,
  type Routes,
  type ServerMessage,
  type SessionInfo,
  type ShowSnapshot,
  type UploadResponse,
} from "@midnightsnack/protocol";
import {
  DirectTransport,
  jsonBody,
  type HostSocket,
  type HostTransport,
  type TransportFailure,
} from "./transport";

export type ConnectionStatus =
  | "connecting"
  | "connected"
  | "disconnected"
  /** Token rejected: the device must pair again. */
  | "unauthorized"
  /** Host speaks a different protocol version. */
  | "incompatible";

export interface ConnectionOptions {
  /** WebSocket URL, e.g. `ws://192.168.1.5:4747/api/v1/ws` (direct connections). */
  wsUrl?: string;
  /** HTTP base for media, e.g. `http://192.168.1.5:4747`; empty for the page's origin. */
  httpBase?: string;
  /** How to reach the host; defaults to a direct connection to `wsUrl`/`httpBase`. */
  transport?: HostTransport;
  token: string;
}

const ACTION_TIMEOUT_MS = 5000;
const PING_INTERVAL_MS = 5000;
/** A remote pointer that stops reporting disappears after this long. */
export const POINTER_IDLE_MS = 3000;

/** Another device's pointer, with the trail of a drawing in progress. */
export interface RemotePointer {
  pos: [number, number];
  mode: PointerMode;
  color: string;
  trail: [number, number][];
  /** `performance.now()` of the last update. */
  at: number;
}

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
  /** Admins: API key and OSC settings. */
  control = $state<ControlSettings | null>(null);
  /** Admins: the API key just created (its token is shown once). */
  newApiKey = $state<{ device_id: string; name: string; token: string } | null>(null);
  /** Admins: uploaded files waiting for a decision. */
  inbox = $state<InboxItem[]>([]);
  autoAcceptUploads = $state(false);
  /** Pointers of other devices, by device id. */
  pointers = $state<Record<string, RemotePointer>>({});
  /** Admins: interfaces, HTTPS and relay status. */
  connectivity = $state<ConnectivityInfo | null>(null);
  /** The OpenSlides meeting shown by OpenSlides cues. */
  openslides = $state<OsMeetingData | null>(null);
  /** Admins: the OpenSlides connection. */
  openslidesStatus = $state<OpenSlidesStatus | null>(null);
  /** Paired remotes: the host's other routes (LAN addresses, relay). */
  routes = $state<Routes | null>(null);
  /** Why the transport failed last (relay: host offline, wrong key…). */
  failure = $state<TransportFailure | null>(null);
  /** Round-trip time of the last ping, in ms. */
  latency = $state<number | null>(null);
  /** Last error reported for an action (cleared after a few seconds). */
  lastError = $state<ErrorCode | null>(null);
  /** `host clock - local clock`, to display host timers correctly. */
  clockOffset = 0;

  #opts: ConnectionOptions;
  #transport: HostTransport;
  #ws: HostSocket | null = null;
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
  #pointerTimer: ReturnType<typeof setInterval> | undefined;
  #pointerQueued: { pos: [number, number] | null; mode: PointerMode; color: string } | null = null;
  #pointerFrame = 0;

  constructor(opts: ConnectionOptions) {
    this.#opts = opts;
    this.#transport = opts.transport ?? new DirectTransport(opts.wsUrl ?? "", opts.httpBase ?? "");
  }

  /** How this connection reaches the host. */
  get transport(): HostTransport {
    return this.#transport;
  }

  connect(): void {
    this.#closed = false;
    clearTimeout(this.#retryTimer);
    this.status = "connecting";
    const ws = this.#transport.openSocket();
    this.#ws = ws;
    ws.onopen = () => {
      this.failure = null;
      this.#send({ type: "hello", protocol_version: PROTOCOL_VERSION, token: this.#opts.token });
    };
    ws.onmessage = (text) => {
      try {
        this.#handle(JSON.parse(text) as ServerMessage);
      } catch (e) {
        console.error("bad message from host", e);
      }
    };
    ws.onclose = (failure) => {
      if (this.#ws !== ws) return;
      this.#ws = null;
      if (failure) this.failure = failure;
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
    clearInterval(this.#pointerTimer);
    this.#ws?.close();
    this.#ws = null;
    this.#transport.dispose();
  }

  #scheduleReconnect(): void {
    if (this.#closed) return;
    clearTimeout(this.#retryTimer);
    this.#retryTimer = setTimeout(() => this.connect(), this.#retryMs);
    this.#retryMs = Math.min(this.#retryMs * 2, 5000);
  }

  #send(msg: ClientMessage): boolean {
    if (!this.#ws?.open) return false;
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
        this.control = msg.control;
        break;
      case "api_key":
        this.newApiKey = { device_id: msg.device_id, name: msg.name, token: msg.token };
        break;
      case "pairing":
        this.pairing = msg.pairing;
        break;
      case "connectivity":
        this.connectivity = msg.connectivity;
        break;
      case "routes":
        this.routes = msg.routes;
        break;
      case "open_slides":
        this.openslides = msg.data;
        break;
      case "open_slides_status":
        this.openslidesStatus = msg.status;
        break;
      case "capture_targets":
        this.captureTargets = msg.targets;
        this.capturePermissionMissing = false;
        break;
      case "render_progress":
        this.renderQueued = msg.queued;
        break;
      case "inbox":
        this.inbox = msg.items;
        this.autoAcceptUploads = msg.auto_accept;
        break;
      case "pointer":
        this.#updatePointer(msg.device_id, msg.pos, msg.mode, msg.color);
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
        if (msg.code === "capture_permission") {
          this.capturePermissionMissing = true;
          this.captureTargets = [];
        } else if (msg.code === "unauthorized") this.status = "unauthorized";
        else if (msg.code === "protocol_mismatch") this.status = "incompatible";
        else this.#flashError(msg.code);
        break;
    }
  }

  #updatePointer(
    deviceId: string,
    pos: [number, number] | null,
    mode: PointerMode,
    color: string,
  ): void {
    if (!pos) {
      delete this.pointers[deviceId];
      return;
    }
    const prev = this.pointers[deviceId];
    const trail = mode === "draw" && prev?.mode === "draw" ? [...prev.trail, pos] : [pos];
    this.pointers[deviceId] = { pos, mode, color, trail, at: performance.now() };
    // Pointers whose device went quiet (or disconnected) fade out.
    this.#pointerTimer ??= setInterval(() => {
      const now = performance.now();
      for (const [id, p] of Object.entries(this.pointers)) {
        if (now - p.at > POINTER_IDLE_MS) delete this.pointers[id];
      }
    }, 1000);
  }

  /**
   * Moves this device's pointer (`pos` as a fraction of the 16:9 slide frame) or hides it
   * (`null`). Updates are coalesced to one per animation frame.
   */
  sendPointer(pos: [number, number] | null, mode: PointerMode, color: string): void {
    const first = this.#pointerQueued === null;
    this.#pointerQueued = { pos, mode, color };
    if (pos === null) {
      // Hiding is sent at once so it is never overtaken by a queued move.
      cancelAnimationFrame(this.#pointerFrame);
      this.#pointerQueued = null;
      this.#send({ type: "pointer", pos, mode, color });
      return;
    }
    if (!first) return;
    this.#pointerFrame = requestAnimationFrame(() => {
      const q = this.#pointerQueued;
      this.#pointerQueued = null;
      if (q) this.#send({ type: "pointer", ...q });
    });
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
    return this.#transport.mediaSrc(
      `/api/v1/media/slide/${encodeURIComponent(pos.cue_id)}/${pos.slide}` +
        `?k=${this.session.media_key}${size}`,
    );
  }

  /** URL of a video/audio cue's file (supports range requests); `null` over the relay. */
  mediaUrl(cueId: string): string | null {
    if (!this.session || !this.#transport.streams) return null;
    return this.#transport.mediaSrc(
      `/api/v1/media/file/${encodeURIComponent(cueId)}?k=${this.session.media_key}`,
    );
  }

  /** MJPEG stream of a capture cue; `null` over the relay. */
  captureUrl(cueId: string, fps?: number): string | null {
    if (!this.session || !this.#transport.streams) return null;
    const rate = fps ? `&fps=${fps}` : "";
    return this.#transport.mediaSrc(
      `/api/v1/media/capture/${encodeURIComponent(cueId)}?k=${this.session.media_key}${rate}`,
    );
  }

  /** Screens and windows available for capture (admins). */
  captureTargets = $state<CaptureTarget[] | null>(null);
  capturePermissionMissing = $state(false);

  /** Admins: creates an API key; the token arrives in `newApiKey`. */
  createApiKey(name: string, role: Role): void {
    this.newApiKey = null;
    this.#send({ type: "create_api_key", name, role });
  }

  requestCaptureTargets(): void {
    this.captureTargets = null;
    this.#send({ type: "list_capture_targets" });
  }

  /**
   * Sends a file to the host's inbox. `onProgress` gets the fraction sent (0–1). Resolves with
   * the host's answer or rejects with an error code.
   */
  async upload(file: File, onProgress?: (fraction: number) => void): Promise<UploadResponse> {
    let res;
    try {
      res = await this.#transport.request(
        "POST",
        `/api/v1/upload?name=${encodeURIComponent(file.name)}`,
        {
          headers: { Authorization: `Bearer ${this.#opts.token}` },
          body: file,
          onUploadProgress: onProgress,
        },
      );
    } catch {
      throw "io" as ErrorCode;
    }
    const body = jsonBody<UploadResponse & { code?: ErrorCode }>(res);
    if (res.status === 200 && body) return body;
    // Not JSON (proxy error page) or an API error.
    throw (body?.code ?? "internal") as ErrorCode;
  }

  /** URL of an image asset (logo, background, logo bug). */
  assetUrl(assetId: string | null | undefined): string | null {
    if (!assetId || !this.session) return null;
    return this.#transport.mediaSrc(
      `/api/v1/media/asset/${encodeURIComponent(assetId)}?k=${this.session.media_key}`,
    );
  }

  cue(id: string | undefined | null) {
    return id ? (this.show?.cues.find((c) => c.id === id) ?? null) : null;
  }

  /** Host time now, in epoch ms. */
  hostNow(): number {
    return Date.now() + this.clockOffset;
  }
}
