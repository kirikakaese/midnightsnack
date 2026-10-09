// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { elapsedMs, formatDuration, parseDuration } from "./time.svelte";

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

  it("parses durations", () => {
    expect(parseDuration("90")).toBe(90_000);
    expect(parseDuration("5:00")).toBe(300_000);
    expect(parseDuration("1:02:03")).toBe(3_723_000);
    expect(parseDuration("0")).toBeNull();
    expect(parseDuration("5:xx")).toBeNull();
    expect(parseDuration("")).toBeNull();
  });
});
