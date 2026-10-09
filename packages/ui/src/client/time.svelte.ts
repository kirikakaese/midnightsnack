// SPDX-License-Identifier: GPL-3.0-or-later
import type { Stopwatch } from "@midnightsnack/protocol";

/** A clock that ticks reactively. Create one per view. */
export class Ticker {
  now = $state(Date.now());
  #timer: ReturnType<typeof setInterval>;

  constructor(intervalMs = 250) {
    this.#timer = setInterval(() => (this.now = Date.now()), intervalMs);
  }

  stop(): void {
    clearInterval(this.#timer);
  }
}

export function elapsedMs(sw: Stopwatch | null | undefined, hostNow: number): number {
  if (!sw) return 0;
  return (
    sw.accumulated_ms +
    (sw.running_since_ms === null ? 0 : Math.max(0, hostNow - sw.running_since_ms))
  );
}

/** `m:ss` or `h:mm:ss`. */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const ss = String(s).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

export function formatClock(epochMs: number, locale?: string): string {
  // A one-off formatter, not reactive state.
  // eslint-disable-next-line svelte/prefer-svelte-reactivity
  return new Date(epochMs).toLocaleTimeString(locale, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

/**
 * Parses `ss`, `m:ss` or `h:mm:ss` into milliseconds. Returns `null` if invalid.
 */
export function parseDuration(text: string): number | null {
  const parts = text.trim().split(":");
  if (parts.length === 0 || parts.length > 3 || parts.some((p) => !/^\d+$/.test(p))) return null;
  const nums = parts.map(Number);
  let seconds = 0;
  for (const n of nums) seconds = seconds * 60 + n;
  return seconds > 0 ? seconds * 1000 : null;
}
