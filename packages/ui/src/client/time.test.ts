// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { elapsedMs, formatDuration } from "./time.svelte";

describe("time", () => {
  it("formats durations", () => {
    expect(formatDuration(0)).toBe("0:00");
    expect(formatDuration(65_000)).toBe("1:05");
    expect(formatDuration(3_725_000)).toBe("1:02:05");
    expect(formatDuration(-5)).toBe("0:00");
  });

  it("computes stopwatch time", () => {
    expect(elapsedMs({ accumulated_ms: 500, running_since_ms: 1000 }, 1500)).toBe(1000);
    expect(elapsedMs({ accumulated_ms: 500, running_since_ms: null }, 9999)).toBe(500);
    expect(elapsedMs(null, 0)).toBe(0);
  });
});
