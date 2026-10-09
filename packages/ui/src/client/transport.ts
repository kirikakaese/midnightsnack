// SPDX-License-Identifier: GPL-3.0-or-later
// How a client reaches its host: directly (LAN, HTTPS, the host's own windows) or through the
// end-to-end encrypted relay tunnel. `HostConnection` and the remote's pairing screen only talk
// to this interface.

/** Why a connection to the host could not be made or was lost. */
export type TransportFailure =
  /** The host is not connected to the relay. */
  | "host_offline"
  /** The host has too many remotes on the relay. */
  | "host_full"
  /** The relay answered with another key: the link is outdated or not for this host. */
  | "host_key"
  | "unreachable";

/** A message channel to the host, shaped like the parts of `WebSocket` the client uses. */
export interface HostSocket {
  readonly open: boolean;
  onopen: (() => void) | null;
  onmessage: ((text: string) => void) | null;
  onclose: ((failure?: TransportFailure) => void) | null;
  send(text: string): void;
  close(): void;
}

export interface HostResponse {
  status: number;
  headers: Record<string, string>;
  body: Uint8Array;
}

export interface RequestOptions {
  headers?: Record<string, string>;
  body?: Blob | Uint8Array | string;
  /** Fraction of the body sent (0–1). */
  onUploadProgress?: (fraction: number) => void;
}

export interface HostTransport {
  readonly kind: "direct" | "relay";
  /** Video/audio files and capture streams can be played (not through the relay). */
  readonly streams: boolean;
  openSocket(): HostSocket;
  request(method: "GET" | "POST", path: string, opts?: RequestOptions): Promise<HostResponse>;
  /**
   * Source for an `<img>` of `path` (an `/api/v1/...` URL path). Direct transports return a
   * URL at once; the relay returns `null` until the image arrived (reactively).
   */
  mediaSrc(path: string): string | null;
  dispose(): void;
}

export function jsonBody<T>(res: HostResponse): T | null {
  try {
    return JSON.parse(new TextDecoder().decode(res.body)) as T;
  } catch {
    return null;
  }
}

class DirectSocket implements HostSocket {
  onopen: (() => void) | null = null;
  onmessage: ((text: string) => void) | null = null;
  onclose: ((failure?: TransportFailure) => void) | null = null;
  #ws: WebSocket | null;

  constructor(url: string) {
    try {
      this.#ws = new WebSocket(url);
    } catch {
      this.#ws = null;
      queueMicrotask(() => this.onclose?.("unreachable"));
      return;
    }
    this.#ws.onopen = () => this.onopen?.();
    this.#ws.onmessage = (ev) => this.onmessage?.(String(ev.data));
    this.#ws.onclose = () => this.onclose?.();
  }

  get open(): boolean {
    return this.#ws?.readyState === WebSocket.OPEN;
  }

  send(text: string): void {
    this.#ws?.send(text);
  }

  close(): void {
    this.#ws?.close();
  }
}

/** Plain WebSocket and HTTP to `httpBase` (empty: the page's own origin). */
export class DirectTransport implements HostTransport {
  readonly kind = "direct";
  readonly streams = true;
  #wsUrl: string;
  #httpBase: string;

  constructor(wsUrl: string, httpBase: string) {
    this.#wsUrl = wsUrl;
    this.#httpBase = httpBase;
  }

  openSocket(): HostSocket {
    return new DirectSocket(this.#wsUrl);
  }

  mediaSrc(path: string): string {
    return `${this.#httpBase}${path}`;
  }

  request(method: "GET" | "POST", path: string, opts: RequestOptions = {}): Promise<HostResponse> {
    // XMLHttpRequest reports upload progress; fetch does not.
    return new Promise((resolve, reject) => {
      const xhr = new XMLHttpRequest();
      xhr.open(method, `${this.#httpBase}${path}`);
      xhr.responseType = "arraybuffer";
      for (const [k, v] of Object.entries(opts.headers ?? {})) xhr.setRequestHeader(k, v);
      if (opts.onUploadProgress) {
        const report = opts.onUploadProgress;
        xhr.upload.onprogress = (e) => {
          if (e.lengthComputable) report(e.loaded / e.total);
        };
      }
      xhr.onload = () => {
        const headers: Record<string, string> = {};
        for (const line of xhr
          .getAllResponseHeaders()
          .trim()
          .split(/[\r\n]+/)) {
          const i = line.indexOf(":");
          if (i > 0) headers[line.slice(0, i).trim().toLowerCase()] = line.slice(i + 1).trim();
        }
        resolve({
          status: xhr.status,
          headers,
          body: new Uint8Array((xhr.response as ArrayBuffer | null) ?? new ArrayBuffer(0)),
        });
      };
      xhr.onerror = () => reject(new Error("network error"));
      xhr.send((opts.body as XMLHttpRequestBodyInit | undefined) ?? null);
    });
  }

  dispose(): void {}
}
