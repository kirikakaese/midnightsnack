// SPDX-License-Identifier: GPL-3.0-or-later
// The remote's end of the relay tunnel: one WebSocket to the relay, a Noise NK session with the
// host inside it, and the tunnel framing on top (stream 0: the WebSocket session; streams 1…:
// HTTP exchanges). See docs/dev/protocol.md.
import {
  RELAY_CLOSE,
  RELAY_PROLOGUE,
  TUNNEL,
  TUNNEL_HEADER,
  TUNNEL_MAX_PAYLOAD,
} from "@midnightsnack/protocol";
import { SvelteMap } from "svelte/reactivity";
import type {
  HostResponse,
  HostSocket,
  HostTransport,
  RequestOptions,
  TransportFailure,
} from "../transport";
import { NkInitiator, NO_AD, NoiseError, type NoiseTransport } from "./noise";

export interface RelayTarget {
  /** Origin of the relay, e.g. `https://relay.example.org`. */
  relayBase: string;
  hostId: string;
  /** The host's static public key. */
  key: Uint8Array;
}

/** Keep at most this much queued in the browser before sending more of an upload. */
const UPLOAD_BUFFER = 512 * 1024;
/** A tunnel carrying a session that hears nothing for this long is considered dead. */
const SILENCE_MS = 20_000;
const HANDSHAKE_TIMEOUT_MS = 10_000;
/** Images kept as blob URLs. */
const MEDIA_CACHE = 48;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

interface Pending {
  resolve: (r: HostResponse) => void;
  reject: (e: Error) => void;
  status: number;
  headers: Record<string, string>;
  chunks: Uint8Array[];
}

function frame(kind: number, stream: number, fin: boolean, payload: Uint8Array): Uint8Array {
  const out = new Uint8Array(TUNNEL_HEADER + payload.length);
  out[0] = kind;
  new DataView(out.buffer).setUint32(1, stream);
  out[5] = fin ? TUNNEL.FIN : 0;
  out.set(payload, TUNNEL_HEADER);
  return out;
}

function joinChunks(chunks: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(chunks.reduce((n, c) => n + c.length, 0));
  let i = 0;
  for (const c of chunks) {
    out.set(c, i);
    i += c.length;
  }
  return out;
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

class TunnelSocket implements HostSocket {
  open = false;
  onopen: (() => void) | null = null;
  onmessage: ((text: string) => void) | null = null;
  onclose: ((failure?: TransportFailure) => void) | null = null;
  tunnel: Tunnel | null = null;
  closed = false;

  send(text: string): void {
    if (this.open) this.tunnel?.sendMessage(text);
  }

  close(): void {
    if (this.closed) return;
    this.closed = true;
    this.tunnel?.detach(this, true);
  }

  /** Called by the tunnel when the session ended. */
  ended(failure?: TransportFailure): void {
    const wasClosed = this.closed;
    this.open = false;
    this.closed = true;
    if (!wasClosed) this.onclose?.(failure);
  }
}

class Tunnel {
  #ws: WebSocket;
  #noise: NoiseTransport | null = null;
  #pending = new Map<number, Pending>();
  #nextStream = 1;
  #message: Uint8Array[] = [];
  #socket: TunnelSocket | null = null;
  #lastHeard = Date.now();
  #watchdog: ReturnType<typeof setInterval>;
  closed = false;
  failure: TransportFailure | undefined;
  onclosed: (() => void) | null = null;

  private constructor(ws: WebSocket) {
    this.#ws = ws;
    this.#watchdog = setInterval(() => {
      if (this.#socket && Date.now() - this.#lastHeard > SILENCE_MS) this.#ws.close();
    }, 5000);
  }

  static open(target: RelayTarget): Promise<Tunnel> {
    const base = target.relayBase.replace(/^http/, "ws");
    const ws = new WebSocket(`${base}/relay/v1/remote/${encodeURIComponent(target.hostId)}`);
    ws.binaryType = "arraybuffer";
    const tunnel = new Tunnel(ws);
    const prologue = encoder.encode(RELAY_PROLOGUE + target.hostId);
    const initiator = new NkInitiator(prologue, target.key);
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => ws.close(), HANDSHAKE_TIMEOUT_MS);
      ws.onopen = () => ws.send(initiator.writeMessage1());
      ws.onmessage = (ev) => {
        if (tunnel.#noise) {
          tunnel.#receive(new Uint8Array(ev.data as ArrayBuffer));
          return;
        }
        clearTimeout(timer);
        try {
          tunnel.#noise = initiator.readMessage2(new Uint8Array(ev.data as ArrayBuffer)).transport;
          tunnel.#lastHeard = Date.now();
          resolve(tunnel);
        } catch (e) {
          tunnel.failure = e instanceof NoiseError ? "host_key" : "unreachable";
          ws.close();
        }
      };
      ws.onclose = (ev) => {
        clearTimeout(timer);
        if (!tunnel.failure) {
          tunnel.failure =
            ev.code === RELAY_CLOSE.HOST_OFFLINE
              ? "host_offline"
              : ev.code === RELAY_CLOSE.HOST_FULL
                ? "host_full"
                : "unreachable";
        }
        if (!tunnel.#noise) reject(tunnel.failure);
        tunnel.#shutdown();
      };
    });
  }

  #shutdown(): void {
    if (this.closed) return;
    this.closed = true;
    clearInterval(this.#watchdog);
    for (const p of this.#pending.values()) p.reject(new Error(this.failure ?? "closed"));
    this.#pending.clear();
    this.#socket?.ended(this.failure);
    this.#socket = null;
    this.onclosed?.();
  }

  close(): void {
    this.#ws.close();
  }

  get bufferedAmount(): number {
    return this.#ws.bufferedAmount;
  }

  #sendFrame(kind: number, stream: number, fin: boolean, payload: Uint8Array): void {
    if (this.closed || !this.#noise) return;
    this.#ws.send(this.#noise.send.encrypt(NO_AD, frame(kind, stream, fin, payload)));
  }

  #receive(data: Uint8Array): void {
    this.#lastHeard = Date.now();
    let plain: Uint8Array;
    try {
      plain = this.#noise!.receive.decrypt(NO_AD, data);
    } catch {
      this.failure = "host_key";
      this.#ws.close();
      return;
    }
    if (plain.length < TUNNEL_HEADER) return;
    const kind = plain[0]!;
    const stream = new DataView(plain.buffer, plain.byteOffset).getUint32(1);
    const fin = (plain[5]! & TUNNEL.FIN) !== 0;
    const payload = plain.subarray(TUNNEL_HEADER);
    switch (kind) {
      case TUNNEL.PING:
        this.#sendFrame(TUNNEL.PONG, stream, true, payload);
        break;
      case TUNNEL.WS_MESSAGE:
        if (stream !== 0) break;
        this.#message.push(payload.slice());
        if (fin) {
          const text = decoder.decode(joinChunks(this.#message));
          this.#message = [];
          this.#socket?.onmessage?.(text);
        }
        break;
      case TUNNEL.WS_CLOSE:
        this.#message = [];
        this.#socket?.ended();
        this.#socket = null;
        break;
      case TUNNEL.RESPONSE_HEAD: {
        const p = this.#pending.get(stream);
        if (!p) break;
        try {
          const head = JSON.parse(decoder.decode(payload)) as {
            status: number;
            headers: [string, string][];
          };
          p.status = head.status;
          p.headers = Object.fromEntries(head.headers.map(([k, v]) => [k.toLowerCase(), v]));
        } catch {
          p.status = 502;
        }
        if (fin) this.#finish(stream);
        break;
      }
      case TUNNEL.RESPONSE_BODY: {
        const p = this.#pending.get(stream);
        if (!p) break;
        p.chunks.push(payload.slice());
        if (fin) this.#finish(stream);
        break;
      }
      case TUNNEL.RESET: {
        const p = this.#pending.get(stream);
        this.#pending.delete(stream);
        p?.reject(new Error("reset"));
        break;
      }
    }
  }

  #finish(stream: number): void {
    const p = this.#pending.get(stream);
    this.#pending.delete(stream);
    p?.resolve({ status: p.status, headers: p.headers, body: joinChunks(p.chunks) });
  }

  attach(socket: TunnelSocket): void {
    if (this.#socket && this.#socket !== socket) this.detach(this.#socket, true);
    this.#socket = socket;
    socket.tunnel = this;
    this.#lastHeard = Date.now();
  }

  detach(socket: TunnelSocket, notifyHost: boolean): void {
    if (this.#socket !== socket) return;
    this.#socket = null;
    this.#message = [];
    if (notifyHost) this.#sendFrame(TUNNEL.WS_CLOSE, 0, true, new Uint8Array(0));
  }

  sendMessage(text: string): void {
    const bytes = encoder.encode(text);
    if (bytes.length === 0) {
      this.#sendFrame(TUNNEL.WS_MESSAGE, 0, true, bytes);
      return;
    }
    for (let i = 0; i < bytes.length; i += TUNNEL_MAX_PAYLOAD) {
      const end = Math.min(i + TUNNEL_MAX_PAYLOAD, bytes.length);
      this.#sendFrame(TUNNEL.WS_MESSAGE, 0, end === bytes.length, bytes.subarray(i, end));
    }
  }

  async request(method: string, path: string, opts: RequestOptions): Promise<HostResponse> {
    const stream = this.#nextStream++;
    const response = new Promise<HostResponse>((resolve, reject) => {
      this.#pending.set(stream, { resolve, reject, status: 0, headers: {}, chunks: [] });
    });
    let body: Blob | Uint8Array | null = null;
    if (typeof opts.body === "string") body = encoder.encode(opts.body);
    else if (opts.body) body = opts.body;
    const size = body ? (body instanceof Blob ? body.size : body.length) : 0;
    const headers: [string, string][] = Object.entries(opts.headers ?? {});
    if (body) headers.push(["content-length", String(size)]);
    const head = encoder.encode(JSON.stringify({ method, path, headers }));
    this.#sendFrame(TUNNEL.REQUEST_HEAD, stream, !body, head);
    if (body) {
      let sent = 0;
      do {
        const end = Math.min(sent + TUNNEL_MAX_PAYLOAD, size);
        const chunk =
          body instanceof Blob
            ? new Uint8Array(await body.slice(sent, end).arrayBuffer())
            : body.subarray(sent, end);
        if (this.closed) break;
        this.#sendFrame(TUNNEL.REQUEST_BODY, stream, end === size, chunk);
        sent = end;
        opts.onUploadProgress?.(size ? sent / size : 1);
        // Pace large uploads to the connection instead of buffering the whole file.
        while (!this.closed && this.#ws.bufferedAmount > UPLOAD_BUFFER) await sleep(15);
      } while (sent < size);
    }
    return response;
  }
}

/** Reaching the host through a relay. */
export class RelayTransport implements HostTransport {
  readonly kind = "relay";
  readonly streams = false;
  /** Why the last tunnel failed, for the UI (`null` while working). */
  failure = $state<TransportFailure | null>(null);
  #target: RelayTarget;
  #tunnel: Tunnel | null = null;
  #opening: Promise<Tunnel> | null = null;
  #media = new SvelteMap<string, string | null>();
  // Plain bookkeeping, not reactive.
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  #loading = new Set<string>();
  #order: string[] = [];
  #disposed = false;

  constructor(target: RelayTarget) {
    this.#target = target;
  }

  #ensure(): Promise<Tunnel> {
    if (this.#tunnel && !this.#tunnel.closed) return Promise.resolve(this.#tunnel);
    this.#opening ??= Tunnel.open(this.#target)
      .then((t) => {
        this.#tunnel = t;
        this.failure = null;
        t.onclosed = () => {
          if (this.#tunnel === t) this.#tunnel = null;
          if (t.failure && t.failure !== "unreachable") this.failure = t.failure;
        };
        return t;
      })
      .catch((f: TransportFailure) => {
        this.failure = f;
        throw f;
      })
      .finally(() => (this.#opening = null));
    return this.#opening;
  }

  openSocket(): HostSocket {
    const socket = new TunnelSocket();
    this.#ensure()
      .then((t) => {
        if (socket.closed) return;
        t.attach(socket);
        socket.open = true;
        socket.onopen?.();
      })
      .catch((f: TransportFailure) => socket.ended(f));
    return socket;
  }

  async request(
    method: "GET" | "POST",
    path: string,
    opts: RequestOptions = {},
  ): Promise<HostResponse> {
    const t = await this.#ensure();
    return t.request(method, path, opts);
  }

  mediaSrc(path: string): string | null {
    const url = this.#media.get(path);
    if (url !== undefined) return url;
    if (!this.#loading.has(path)) {
      this.#loading.add(path);
      // Not during the reactive read that asked for it.
      queueMicrotask(() => void this.#load(path));
    }
    return null;
  }

  async #load(path: string): Promise<void> {
    try {
      const res = await this.request("GET", path);
      if (this.#disposed) return;
      if (res.status !== 200) throw new Error(String(res.status));
      const blob = new Blob([res.body as BlobPart], {
        type: res.headers["content-type"] ?? "application/octet-stream",
      });
      this.#remember(path, URL.createObjectURL(blob));
    } catch {
      // Retry on the next request for it after a moment.
      setTimeout(() => this.#loading.delete(path), 3000);
      return;
    }
    this.#loading.delete(path);
  }

  #remember(path: string, url: string): void {
    this.#media.set(path, url);
    this.#order.push(path);
    while (this.#order.length > MEDIA_CACHE) {
      const old = this.#order.shift()!;
      const oldUrl = this.#media.get(old);
      this.#media.delete(old);
      if (oldUrl) URL.revokeObjectURL(oldUrl);
    }
  }

  dispose(): void {
    this.#disposed = true;
    this.#tunnel?.close();
    for (const url of this.#media.values()) if (url) URL.revokeObjectURL(url);
    this.#media.clear();
  }
}
