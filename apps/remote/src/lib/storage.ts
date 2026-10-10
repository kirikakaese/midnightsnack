// SPDX-License-Identifier: GPL-3.0-or-later
// How this page reaches its host, and what survives reloads: the session token (so a paired
// phone reconnects without pairing again), the device name and the host's other routes.
import type { Routes } from "@midnightsnack/protocol";
import {
  DirectTransport,
  RelayTransport,
  base64urlDecode,
  type HostTransport,
} from "@midnightsnack/ui";

const TOKEN_KEY = "midnightsnack.token";
const NAME_KEY = "midnightsnack.deviceName";
const ROUTES_KEY = "midnightsnack.routes";
const LANGUAGE_KEY = "midnightsnack.language";

function get(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function set(key: string, value: string | null): void {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
  } catch {
    // Private mode or storage disabled: the session simply lasts until reload.
  }
}

/** The language chosen on this phone; `null` follows the browser. */
export const loadLanguage = (): string | null => get(LANGUAGE_KEY);
export const saveLanguage = (code: string | null): void => set(LANGUAGE_KEY, code);

/** Where the host is: on this page's origin (LAN, HTTPS) or behind the relay serving the page. */
export type HostLink = { kind: "direct" } | { kind: "relay"; hostId: string; key: string | null };

function fragmentParam(name: string): string | null {
  const m = new RegExp(`[#&]${name}=([^&]+)`).exec(window.location.hash);
  return m?.[1] ? decodeURIComponent(m[1]) : null;
}

/** Reads the link from the URL; a relay key in the fragment is remembered for reloads. */
export function currentLink(): HostLink {
  const m = /^\/r\/([A-Za-z0-9_-]{8,64})\/?$/.exec(window.location.pathname);
  if (!m) return { kind: "direct" };
  const hostId = m[1]!;
  // A host's key never changes for its id (a new relay identity is a new id), so a key once
  // stored wins over one in a link: a crafted link cannot swap it.
  const keyName = `midnightsnack.relay.${hostId}.key`;
  const stored = get(keyName);
  const fromUrl = fragmentParam("k");
  if (fromUrl && !stored) set(keyName, fromUrl);
  return { kind: "relay", hostId, key: stored ?? fromUrl };
}

function tokenKey(link: HostLink): string {
  return link.kind === "relay" ? `midnightsnack.relay.${link.hostId}.token` : TOKEN_KEY;
}

export const storage = {
  token: (link: HostLink) => get(tokenKey(link)),
  setToken: (link: HostLink, t: string | null) => set(tokenKey(link), t),
  deviceName: () => get(NAME_KEY),
  setDeviceName: (n: string) => set(NAME_KEY, n),
  routes: (): Routes | null => {
    try {
      return JSON.parse(get(ROUTES_KEY) ?? "null") as Routes | null;
    } catch {
      return null;
    }
  },
  setRoutes: (r: Routes) => set(ROUTES_KEY, JSON.stringify(r)),
};

/** Join token from `…#t=<token>`, if present. */
export function joinTokenFromUrl(): string | null {
  return fragmentParam("t");
}

/** A session token handed over when switching between the LAN and the relay (`…#a=<token>`). */
export function handedOverToken(): string | null {
  return fragmentParam("a");
}

/** Removes secrets from the address bar and history (keeps the relay page path). */
export function clearJoinUrl(): void {
  const path = window.location.pathname.startsWith("/r/") ? window.location.pathname : "/";
  history.replaceState(null, "", path);
}

export function wsUrl(): string {
  const proto = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${window.location.host}/api/v1/ws`;
}

/** A transport for the link; `null` if a relay link lacks the host's key. */
export function makeTransport(link: HostLink): HostTransport | null {
  if (link.kind === "direct") return new DirectTransport(wsUrl(), "");
  if (!link.key) return null;
  let key: Uint8Array;
  try {
    key = base64urlDecode(link.key);
  } catch {
    return null;
  }
  if (key.length !== 32) return null;
  return new RelayTransport({ relayBase: window.location.origin, hostId: link.hostId, key });
}

/** Opens the host's relay page, carrying this device's token. */
export function switchToRelay(route: { url: string; key: string }, token: string): void {
  window.location.href = `${route.url}#k=${route.key}&a=${encodeURIComponent(token)}`;
}

/** Opens the host on the local network, carrying this device's token. */
export function switchToLan(base: string, token: string): void {
  window.location.href = `${base}/#a=${encodeURIComponent(token)}`;
}

export function guessDeviceName(fallback: string): string {
  const ua = navigator.userAgent;
  if (/iPad/.test(ua)) return "iPad";
  if (/iPhone/.test(ua)) return "iPhone";
  if (/Android/.test(ua)) return "Android";
  if (/Macintosh/.test(ua)) return "Mac";
  if (/Windows/.test(ua)) return "Windows PC";
  if (/Linux/.test(ua)) return "Linux PC";
  return fallback;
}
