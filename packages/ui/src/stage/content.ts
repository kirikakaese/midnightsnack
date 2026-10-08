// SPDX-License-Identifier: GPL-3.0-or-later
// Turns a position into a description of what to draw, independent of how it is drawn.
import type {
  CueSummary,
  MediaOptions,
  Position,
  ShowSnapshot,
  TextTheme,
  TimerCue,
  Transition,
} from "@midnightsnack/protocol";
import type { HostConnection } from "../client/connection.svelte";

export type Content =
  | { kind: "image"; key: string; src: string; background: string }
  | { kind: "blank"; key: string; background: string }
  | {
      kind: "media";
      key: string;
      cueId: string;
      video: boolean;
      src: string;
      options: MediaOptions;
      durationMs: number | null;
      name: string;
    }
  | { kind: "text"; key: string; text: string; theme: TextTheme; backgroundSrc: string | null }
  | {
      kind: "timer";
      key: string;
      cueId: string;
      timer: TimerCue;
      theme: TextTheme;
      backgroundSrc: string | null;
    };

export function effectiveTheme(show: ShowSnapshot, cue: CueSummary): TextTheme {
  return cue.text?.theme ?? cue.timer?.theme ?? show.default_theme;
}

export function effectiveTransition(show: ShowSnapshot | null, cue: CueSummary | null): Transition {
  return cue?.transition ?? show?.default_transition ?? { kind: "cut", duration_ms: 0 };
}

export function describe(
  conn: HostConnection,
  pos: Position | null | undefined,
  width?: number,
  height?: number,
): Content | null {
  const show = conn.show;
  if (!pos || !show) return null;
  const cue = show.cues.find((c) => c.id === pos.cue_id);
  if (!cue || pos.slide >= cue.slide_count) return null;
  const key = `${cue.id}:${pos.slide}`;
  switch (cue.kind) {
    case "blank":
      return { kind: "blank", key, background: cue.background ?? "#000" };
    case "video":
    case "audio": {
      const src = conn.mediaUrl(cue.id);
      if (!src || !cue.media) return null;
      return {
        kind: "media",
        key,
        cueId: cue.id,
        video: cue.kind === "video",
        src,
        options: cue.media.options,
        durationMs: cue.media.duration_ms,
        name: cue.name,
      };
    }
    case "text": {
      const theme = effectiveTheme(show, cue);
      return {
        kind: "text",
        key,
        text: cue.text?.slides[pos.slide] ?? "",
        theme,
        backgroundSrc: conn.assetUrl(theme.background_image),
      };
    }
    case "timer": {
      if (!cue.timer) return null;
      const theme = effectiveTheme(show, cue);
      return {
        kind: "timer",
        key,
        cueId: cue.id,
        timer: cue.timer,
        theme,
        backgroundSrc: conn.assetUrl(theme.background_image),
      };
    }
    default: {
      const src = conn.slideUrl(pos, width, height);
      return src ? { kind: "image", key, src, background: "#000" } : null;
    }
  }
}

/** Position in the media file (ms) at `hostNow`, honoring trim and looping. */
export function mediaPosition(
  stopwatch: { accumulated_ms: number; running_since_ms: number | null },
  options: MediaOptions,
  durationMs: number | null,
  hostNow: number,
): number {
  const raw =
    stopwatch.accumulated_ms +
    (stopwatch.running_since_ms === null ? 0 : Math.max(0, hostNow - stopwatch.running_since_ms));
  const end = options.end_ms ?? durationMs;
  if (end === null || end <= options.start_ms) return raw;
  if (options.loop) {
    const span = end - options.start_ms;
    return options.start_ms + ((((raw - options.start_ms) % span) + span) % span);
  }
  return Math.min(raw, end);
}

/** Milliseconds until a `HH:MM` local time today (negative once passed). */
export function untilClockTime(time: string, now: Date): number {
  const [h, m] = time.split(":").map(Number);
  const target = new Date(now);
  target.setHours(h ?? 0, m ?? 0, 0, 0);
  return target.getTime() - now.getTime();
}
