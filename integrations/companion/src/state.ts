// SPDX-License-Identifier: GPL-3.0-or-later
// Pure mapping between the host's state and Companion actions, feedbacks and variables.
import type { Action, LiveState, ShowSnapshot, Stopwatch } from "@midnightsnack/protocol";

/** Companion action ids → protocol actions (actions without options). */
export const SIMPLE_ACTIONS = {
  go: "go",
  next: "next",
  prev: "prev",
  next_cue: "next_cue",
  prev_cue: "prev_cue",
  panic: "panic",
  clear_drawing: "clear_drawing",
  media_play: "media_play",
  media_pause: "media_pause",
  media_restart: "media_restart",
  timer_start: "timer_start",
  timer_pause: "timer_pause",
  timer_reset: "timer_reset",
  countdown_start: "countdown_start",
  countdown_pause: "countdown_pause",
  countdown_reset: "countdown_reset",
} as const satisfies Record<string, Action["action"]>;

export type Master = "blackout" | "freeze" | "logo";
export type Switch = "toggle" | "on" | "off";

/** The protocol action for a master button. */
export function masterAction(master: Master, mode: Switch): Action {
  if (mode === "toggle") {
    return { action: `toggle_${master}` } as Action;
  }
  return { action: `set_${master}`, on: mode === "on" } as Action;
}

/** Go to cue `number` (1-based) and `slide` (1-based); `null` if there is no such cue. */
export function goToCue(show: ShowSnapshot | null, number: number, slide = 1): Action | null {
  const cue = show?.cues[Math.round(number) - 1];
  if (!cue || cue.slide_count === 0) return null;
  const s = Math.min(Math.max(1, Math.round(slide)), cue.slide_count) - 1;
  return { action: "go_to", position: { cue_id: cue.id, slide: s } };
}

export function overlayAction(overlayId: string, mode: Switch): Action {
  return mode === "toggle"
    ? { action: "toggle_overlay", overlay_id: overlayId }
    : { action: "set_overlay_visible", overlay_id: overlayId, visible: mode === "on" };
}

function elapsed(sw: Stopwatch | undefined, now: number): number {
  if (!sw) return 0;
  return (
    sw.accumulated_ms + (sw.running_since_ms !== null ? Math.max(0, now - sw.running_since_ms) : 0)
  );
}

/** `m:ss` (or `h:mm:ss`), with `-` in overtime. */
export function clock(ms: number): string {
  const sign = ms < 0 ? "-" : "";
  const s = Math.floor(Math.abs(ms) / 1000);
  const mm = String(Math.floor(s / 60) % 60).padStart(2, "0");
  const ss = String(s % 60).padStart(2, "0");
  return s >= 3600
    ? `${sign}${Math.floor(s / 3600)}:${mm}:${ss}`
    : `${sign}${Math.floor(s / 60)}:${ss}`;
}

export const VARIABLES = {
  show_title: "Show title",
  cue_name: "Live cue name",
  cue_number: "Live cue number",
  slide: "Slide number",
  slide_count: "Slides in the live cue",
  slide_of: "Slide n / m",
  next_name: "Next cue name",
  show_timer: "Show timer",
  slide_timer: "Slide timer",
  countdown: "Countdown remaining",
  countdown_label: "Countdown label",
  stage_message: "Message to stage",
} as const;

export type VariableValues = Record<keyof typeof VARIABLES, string | number>;

/** Variable values for `now` (host clock, epoch ms). */
export function variables(
  show: ShowSnapshot | null,
  live: LiveState | null,
  now: number,
): VariableValues {
  const program = live?.program ?? null;
  const index = program ? (show?.cues.findIndex((c) => c.id === program.cue_id) ?? -1) : -1;
  const cue = index >= 0 ? show!.cues[index]! : null;
  const next = live?.next ? show?.cues.find((c) => c.id === live.next!.cue_id) : undefined;
  const countdownLeft = live
    ? live.countdown.duration_ms - elapsed(live.countdown.elapsed, now)
    : 0;
  return {
    show_title: show?.title ?? "",
    cue_name: cue?.name ?? "",
    cue_number: cue ? index + 1 : 0,
    slide: cue && program ? program.slide + 1 : 0,
    slide_count: cue?.slide_count ?? 0,
    slide_of: cue && program ? `${program.slide + 1}/${cue.slide_count}` : "",
    next_name: next?.name ?? "",
    show_timer: clock(elapsed(live?.show_timer, now)),
    slide_timer: clock(elapsed(live?.slide_timer, now)),
    countdown: clock(countdownLeft),
    countdown_label: live?.countdown.label ?? "",
    stage_message: live?.stage_message ?? "",
  };
}

export function masterActive(live: LiveState | null, master: Master): boolean {
  return !!live?.masters[master];
}

/** The cue with this 1-based number is on the program. */
export function cueLive(
  show: ShowSnapshot | null,
  live: LiveState | null,
  number: number,
): boolean {
  const cue = show?.cues[Math.round(number) - 1];
  return !!cue && live?.program?.cue_id === cue.id;
}

export function overlayVisible(live: LiveState | null, overlayId: string): boolean {
  return !!live?.overlays_visible.includes(overlayId);
}

export function countdownOvertime(live: LiveState | null, now: number): boolean {
  return !!live && live.countdown.duration_ms - elapsed(live.countdown.elapsed, now) < 0;
}
