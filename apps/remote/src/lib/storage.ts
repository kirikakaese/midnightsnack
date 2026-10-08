// SPDX-License-Identifier: GPL-3.0-or-later
// The session token survives reloads so a paired phone reconnects without pairing again.
const TOKEN_KEY = "midnightsnack.token";
const NAME_KEY = "midnightsnack.deviceName";

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

export const storage = {
  token: () => get(TOKEN_KEY),
  setToken: (t: string | null) => set(TOKEN_KEY, t),
  deviceName: () => get(NAME_KEY),
  setDeviceName: (n: string) => set(NAME_KEY, n),
};

/** Join token from `/join#t=<token>`, if present. */
export function joinTokenFromUrl(): string | null {
  const m = /[#&]t=([^&]+)/.exec(window.location.hash);
  return m?.[1] ? decodeURIComponent(m[1]) : null;
}

export function clearJoinUrl(): void {
  history.replaceState(null, "", "/");
}

export function wsUrl(): string {
  const proto = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${window.location.host}/api/v1/ws`;
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
