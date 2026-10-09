// SPDX-License-Identifier: GPL-3.0-or-later
// What an OpenSlides cue shows right now: resolves the cue's slide (including "follow
// projector") against the live meeting data and picks the page.
import {
  OS_AGENDA_PAGE_SIZE,
  type OpenSlidesSlide,
  type OsAgendaItem,
  type OsBlock,
  type OsMeetingData,
  type OsMotion,
  type OsSpeaker,
  type OsSpeakerList,
  type OsTopic,
} from "@midnightsnack/protocol";

/** Waiting speakers shown on a list of speakers slide. */
export const NEXT_SPEAKERS = 5;
/** Finished speakers shown above the current one. */
export const LAST_SPEAKERS = 2;

export type OsView =
  | { kind: "agenda"; items: OsAgendaItem[]; page: number; pages: number }
  | {
      kind: "motion";
      motion: OsMotion;
      blocks: OsBlock[];
      first: boolean;
      page: number;
      pages: number;
    }
  | {
      kind: "topic";
      topic: OsTopic;
      blocks: OsBlock[];
      first: boolean;
      page: number;
      pages: number;
    }
  | {
      kind: "speakers";
      list: OsSpeakerList;
      last: OsSpeaker[];
      current: OsSpeaker[];
      next: OsSpeaker[];
      /** Waiting speakers beyond `next`. */
      more: number;
    }
  | { kind: "other"; collection: string; title: string }
  | { kind: "empty"; reason: "not_connected" | "not_found" | "nothing_projected" | "no_list" };

function paged<T>(starts: number[], items: T[], page: number): { items: T[]; page: number } {
  const p = Math.min(Math.max(0, page), Math.max(0, starts.length - 1));
  const from = starts[p] ?? 0;
  const to = starts[p + 1] ?? items.length;
  return { items: items.slice(from, to), page: p };
}

function speakers(list: OsSpeakerList): OsView {
  const finished = list.speakers.filter((s) => s.state === "finished");
  const current = list.speakers.filter((s) => s.state === "speaking");
  const waiting = list.speakers.filter((s) => s.state === "waiting");
  return {
    kind: "speakers",
    list,
    last: finished.slice(-LAST_SPEAKERS),
    current,
    next: waiting.slice(0, NEXT_SPEAKERS),
    more: Math.max(0, waiting.length - NEXT_SPEAKERS),
  };
}

export function resolveOpenSlides(
  data: OsMeetingData | null,
  slide: OpenSlidesSlide,
  page: number,
): OsView {
  if (!data) return { kind: "empty", reason: "not_connected" };
  switch (slide.kind) {
    case "agenda": {
      const pages = Math.max(1, Math.ceil(data.agenda.length / OS_AGENDA_PAGE_SIZE));
      const p = Math.min(Math.max(0, page), pages - 1);
      return {
        kind: "agenda",
        items: data.agenda.slice(p * OS_AGENDA_PAGE_SIZE, (p + 1) * OS_AGENDA_PAGE_SIZE),
        page: p,
        pages,
      };
    }
    case "motion": {
      const motion = data.motions.find((m) => m.id === slide.motion_id);
      if (!motion) return { kind: "empty", reason: "not_found" };
      const { items, page: p } = paged(motion.page_starts, motion.body, page);
      return {
        kind: "motion",
        motion,
        blocks: items,
        first: p === 0,
        page: p,
        pages: Math.max(1, motion.page_starts.length),
      };
    }
    case "topic": {
      const topic = data.topics.find((t) => t.id === slide.topic_id);
      if (!topic) return { kind: "empty", reason: "not_found" };
      const { items, page: p } = paged(topic.page_starts, topic.body, page);
      return {
        kind: "topic",
        topic,
        blocks: items,
        first: p === 0,
        page: p,
        pages: Math.max(1, topic.page_starts.length),
      };
    }
    case "speakers": {
      const id = slide.list_id ?? data.current_list_id;
      if (id === null) return { kind: "empty", reason: "no_list" };
      const list = data.lists.find((l) => l.id === id);
      return list ? speakers(list) : { kind: "empty", reason: "not_found" };
    }
    case "follow": {
      const pid = slide.projector_id ?? data.reference_projector_id;
      const projector = data.projectors.find((p) => p.id === pid);
      if (!projector) return { kind: "empty", reason: "not_found" };
      const current = projector.current;
      if (!current) return { kind: "empty", reason: "nothing_projected" };
      switch (current.kind) {
        case "agenda":
          return resolveOpenSlides(data, { kind: "agenda" }, 0);
        case "motion":
          return resolveOpenSlides(data, { kind: "motion", motion_id: current.motion_id }, 0);
        case "topic":
          return resolveOpenSlides(data, { kind: "topic", topic_id: current.topic_id }, 0);
        case "speakers":
          return resolveOpenSlides(data, { kind: "speakers", list_id: current.list_id }, 0);
        case "current_speakers":
          return resolveOpenSlides(data, { kind: "speakers", list_id: null }, 0);
        case "other":
          return { kind: "other", collection: current.collection, title: current.title };
      }
    }
  }
}
