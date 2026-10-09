// SPDX-License-Identifier: GPL-3.0-or-later
// "Join this Wi-Fi" QR codes (the `WIFI:` format phones understand) and hotspot passwords.

/** Escapes `\ ; , : "` as the WIFI: format requires. */
function escape(s: string): string {
  return s.replace(/([\\;,:"])/g, "\\$1");
}

/** `WIFI:T:WPA;S:<ssid>;P:<password>;;`, or `null` if the values cannot be a WPA2 network. */
export function wifiQrText(ssid: string, password: string): string | null {
  if (!ssid || ssid.length > 32 || password.length < 8 || password.length > 63) return null;
  return `WIFI:T:WPA;S:${escape(ssid)};P:${escape(password)};;`;
}

/** 12 characters that are easy to read and type (no 0/O, 1/l/I). */
export function randomPassword(): string {
  const alphabet = "abcdefghijkmnpqrstuvwxyz23456789";
  const bytes = crypto.getRandomValues(new Uint8Array(12));
  return Array.from(bytes, (b) => alphabet[b % alphabet.length]).join("");
}
