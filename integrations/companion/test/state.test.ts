// SPDX-License-Identifier: GPL-3.0-or-later
import type { CueSummary, LiveState, ShowSnapshot } from "@midnightsnack/protocol";
import { PROTOCOL_VERSION as PROTOCOL } from "@midnightsnack/protocol";
import { describe, expect, it } from "vitest";
import { PROTOCOL_VERSION } from "../src/client";
import {
  clock,
  countdownOvertime,
  cueLive,
  goToCue,
  masterAction,
  overlayAction,
  variables,
} from "../src/state";

function cue(id: string, name: string, slides: number): CueSummary {
  return {
    id,
    name,
    kind: "pdf",
    slide_count: slides,
    color: null,
    notes: "",
    slide_notes: [],
    background: null,
    transition: null,
    auto_advance_ms: null,
    media: null,
    text: null,
    timer: null,
    web: null,
    capture: null,
    openslides: null,
    targets: null,
    converted_from: null,
  };
}

const show = {
  title: "Gala",
  cues: [cue("a", "Welcome", 3), cue("b", "Keynote", 10)],
  overlays: [],
} as unknown as ShowSnapshot;

function live(patch: Partial<LiveState> = {}): LiveState {
  return {
    program: { cue_id: "b", slide: 4 },
    output: { cue_id: "b", slide: 4 },
    next: { cue_id: "b", slide: 5 },
    prev: null,
    masters: { blackout: false, freeze: false, logo: false },
    show_timer: { accumulated_ms: 60_000, running_since_ms: 1_000 },
    slide_timer: { accumulated_ms: 0, running_since_ms: null },
    media: null,
    countdown: {
      duration_ms: 120_000,
      label: "Talk",
      elapsed: { accumulated_ms: 0, running_since_ms: 0 },
    },
    overlays_visible: ["clock"],
    stage_message: null,
    outputs: [],
    test_pattern: null,
    capture_lost: [],
    web_nav: null,
    drawing: null,
    auto_advance_at_ms: null,
    host_time_ms: 0,
    revision: 1,
    ...patch,
  };
}

describe("companion state mapping", () => {
  it("speaks the host's protocol version", () => {
    expect(PROTOCOL_VERSION).toBe(PROTOCOL);
  });

  it("builds variables from the live state", () => {
    const v = variables(show, live(), 31_000);
    expect(v.cue_name).toBe("Keynote");
    expect(v.cue_number).toBe(2);
    expect(v.slide_of).toBe("5/10");
    expect(v.next_name).toBe("Keynote");
    expect(v.show_timer).toBe("1:30");
    expect(v.countdown).toBe("1:29");
    expect(v.countdown_label).toBe("Talk");
    const empty = variables(null, null, 0);
    expect(empty.cue_name).toBe("");
    expect(empty.slide).toBe(0);
  });

  it("maps buttons to actions", () => {
    expect(masterAction("blackout", "toggle")).toEqual({ action: "toggle_blackout" });
    expect(masterAction("freeze", "on")).toEqual({ action: "set_freeze", on: true });
    expect(overlayAction("clock", "off")).toEqual({
      action: "set_overlay_visible",
      overlay_id: "clock",
      visible: false,
    });
    expect(goToCue(show, 2, 99)).toEqual({
      action: "go_to",
      position: { cue_id: "b", slide: 9 },
    });
    expect(goToCue(show, 3)).toBeNull();
  });

  it("evaluates feedbacks", () => {
    expect(cueLive(show, live(), 2)).toBe(true);
    expect(cueLive(show, live(), 1)).toBe(false);
    expect(countdownOvertime(live(), 121_000)).toBe(true);
    expect(countdownOvertime(live(), 10_000)).toBe(false);
  });

  it("formats clocks", () => {
    expect(clock(0)).toBe("0:00");
    expect(clock(-5_000)).toBe("-0:05");
    expect(clock(3_725_000)).toBe("1:02:05");
  });
});
