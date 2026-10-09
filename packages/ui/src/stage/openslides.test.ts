// SPDX-License-Identifier: GPL-3.0-or-later
import type { OsMeetingData, OsSpeaker } from "@midnightsnack/protocol";
import { describe, expect, it } from "vitest";
import { resolveOpenSlides } from "./openslides";

const speaker = (name: string, state: OsSpeaker["state"]): OsSpeaker => ({
  name,
  state,
  speech_state: null,
  point_of_order: false,
  begin_ms: state === "waiting" ? null : 1000,
  end_ms: state === "finished" ? 2000 : null,
});

const data: OsMeetingData = {
  meeting_id: 1,
  name: "Assembly",
  agenda: Array.from({ length: 14 }, (_, i) => ({
    id: i + 1,
    number: `TOP ${i + 1}`,
    title: `Item ${i + 1}`,
    level: 0,
    closed: false,
    type: "common" as const,
  })),
  motions: [
    {
      id: 1,
      number: "A1",
      title: "Budget",
      state: "submitted",
      submitters: ["Ada"],
      body: ["a", "b", "c"].map((text) => ({ kind: "paragraph" as const, text })),
      page_starts: [0, 2],
      list_of_speakers_id: 7,
    },
  ],
  topics: [],
  lists: [
    {
      id: 7,
      title: "A1 · Budget",
      closed: false,
      speakers: [
        speaker("Old", "finished"),
        speaker("Older", "finished"),
        speaker("Oldest", "finished"),
        speaker("Now", "speaking"),
        ...Array.from({ length: 7 }, (_, i) => speaker(`Next ${i}`, "waiting")),
      ],
    },
  ],
  projectors: [
    { id: 1, name: "Main", current: { kind: "motion", motion_id: 1 } },
    { id: 2, name: "Side", current: { kind: "other", collection: "poll", title: "Vote" } },
    { id: 3, name: "Empty", current: null },
  ],
  reference_projector_id: 1,
  current_list_id: 7,
};

describe("resolveOpenSlides", () => {
  it("pages the agenda and motions", () => {
    const a = resolveOpenSlides(data, { kind: "agenda" }, 1);
    expect(a).toMatchObject({ kind: "agenda", page: 1, pages: 2 });
    expect(a.kind === "agenda" && a.items.map((i) => i.id)).toEqual([13, 14]);
    const m0 = resolveOpenSlides(data, { kind: "motion", motion_id: 1 }, 0);
    const m1 = resolveOpenSlides(data, { kind: "motion", motion_id: 1 }, 5);
    expect(m0.kind === "motion" && m0.blocks.map((b) => b.text)).toEqual(["a", "b"]);
    expect(m1).toMatchObject({ kind: "motion", page: 1, first: false, pages: 2 });
    expect(m1.kind === "motion" && m1.blocks.map((b) => b.text)).toEqual(["c"]);
  });

  it("shows the current list of speakers with recent and upcoming speakers", () => {
    const v = resolveOpenSlides(data, { kind: "speakers", list_id: null }, 0);
    expect(v.kind).toBe("speakers");
    if (v.kind !== "speakers") return;
    expect(v.last.map((s) => s.name)).toEqual(["Older", "Oldest"]);
    expect(v.current.map((s) => s.name)).toEqual(["Now"]);
    expect(v.next).toHaveLength(5);
    expect(v.more).toBe(2);
  });

  it("follows projectors", () => {
    expect(resolveOpenSlides(data, { kind: "follow", projector_id: null }, 0).kind).toBe("motion");
    expect(resolveOpenSlides(data, { kind: "follow", projector_id: 2 }, 0)).toEqual({
      kind: "other",
      collection: "poll",
      title: "Vote",
    });
    expect(resolveOpenSlides(data, { kind: "follow", projector_id: 3 }, 0)).toEqual({
      kind: "empty",
      reason: "nothing_projected",
    });
  });

  it("reports missing data", () => {
    expect(resolveOpenSlides(null, { kind: "agenda" }, 0)).toEqual({
      kind: "empty",
      reason: "not_connected",
    });
    expect(resolveOpenSlides(data, { kind: "motion", motion_id: 9 }, 0)).toEqual({
      kind: "empty",
      reason: "not_found",
    });
    expect(
      resolveOpenSlides({ ...data, current_list_id: null }, { kind: "speakers", list_id: null }, 0),
    ).toEqual({ kind: "empty", reason: "no_list" });
  });
});
