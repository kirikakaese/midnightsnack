// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { mediaPosition, untilClockTime } from "./content";

const opts = { loop: false, start_ms: 1000, end_ms: null, volume: 1, auto_advance: false };

describe("mediaPosition", () => {
  it("advances while running and clamps at the end", () => {
    const sw = { accumulated_ms: 1000, running_since_ms: 0 };
    expect(mediaPosition(sw, opts, 5000, 2500)).toBe(3500);
    expect(mediaPosition(sw, opts, 5000, 99_000)).toBe(5000);
    expect(mediaPosition({ accumulated_ms: 2000, running_since_ms: null }, opts, 5000, 9e9)).toBe(
      2000,
    );
  });

  it("wraps when looping within the trim", () => {
    const loop = { ...opts, loop: true, end_ms: 3000 };
    const sw = { accumulated_ms: 1000, running_since_ms: 0 };
    expect(mediaPosition(sw, loop, null, 1500)).toBe(2500);
    expect(mediaPosition(sw, loop, null, 2500)).toBe(1500);
  });

  it("ignores unknown durations", () => {
    expect(mediaPosition({ accumulated_ms: 0, running_since_ms: 0 }, opts, null, 7000)).toBe(7000);
  });
});

describe("untilClockTime", () => {
  it("counts to a time today", () => {
    const now = new Date(2026, 9, 8, 19, 0, 0);
    expect(untilClockTime("19:30", now)).toBe(30 * 60 * 1000);
    expect(untilClockTime("18:59", now)).toBe(-60 * 1000);
  });
});
