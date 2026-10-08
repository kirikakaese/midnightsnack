// SPDX-License-Identifier: GPL-3.0-or-later
// Keyboard and presentation-clicker mapping. Clickers emulate keys (arrows, PageUp/PageDown,
// F5, Escape, "." or "b"), so they are handled exactly like the keyboard.
import type { Action } from "@midnightsnack/protocol";

export type KeyAction =
  | "go"
  | "next"
  | "prev"
  | "next_cue"
  | "prev_cue"
  | "toggle_blackout"
  | "toggle_freeze"
  | "toggle_logo"
  | "panic"
  | "none";

export const DEFAULT_KEYMAP: Record<string, KeyAction> = {
  " ": "next",
  ArrowRight: "next",
  ArrowDown: "next",
  PageDown: "next",
  ArrowLeft: "prev",
  ArrowUp: "prev",
  PageUp: "prev",
  Enter: "go",
  F5: "go",
  "Shift+F5": "go",
  "Shift+ArrowRight": "next_cue",
  "Shift+ArrowLeft": "prev_cue",
  b: "toggle_blackout",
  ".": "toggle_blackout",
  f: "toggle_freeze",
  l: "toggle_logo",
  Escape: "panic",
};

export const KEY_ACTIONS: KeyAction[] = [
  "go",
  "next",
  "prev",
  "next_cue",
  "prev_cue",
  "toggle_blackout",
  "toggle_freeze",
  "toggle_logo",
  "panic",
  "none",
];

/** Normalized key name: single letters lower-case, modifiers (except Shift on letters) prefixed. */
export function keyName(
  e: Pick<KeyboardEvent, "key" | "shiftKey" | "ctrlKey" | "altKey" | "metaKey">,
): string {
  const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.metaKey) mods.push("Meta");
  // Shift is implied by printable characters ("." vs ">"), so only record it for named keys.
  if (e.shiftKey && e.key.length > 1) mods.push("Shift");
  return [...mods, key].join("+");
}

export function toAction(a: KeyAction): Action | null {
  switch (a) {
    case "go":
      return { action: "go" };
    case "next":
      return { action: "next" };
    case "prev":
      return { action: "prev" };
    case "next_cue":
      return { action: "next_cue" };
    case "prev_cue":
      return { action: "prev_cue" };
    case "toggle_blackout":
      return { action: "toggle_blackout" };
    case "toggle_freeze":
      return { action: "toggle_freeze" };
    case "toggle_logo":
      return { action: "toggle_logo" };
    case "panic":
      return { action: "panic" };
    case "none":
      return null;
  }
}

export function buildKeymap(overrides: Record<string, string>): Record<string, KeyAction> {
  const map = { ...DEFAULT_KEYMAP };
  for (const [k, v] of Object.entries(overrides)) {
    if ((KEY_ACTIONS as string[]).includes(v)) map[k] = v as KeyAction;
  }
  return map;
}

function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return (
    target.isContentEditable ||
    target.tagName === "INPUT" ||
    target.tagName === "TEXTAREA" ||
    target.tagName === "SELECT"
  );
}

/**
 * Installs a window keydown handler. Returns a cleanup function.
 * Keys typed into form fields are ignored, and so is auto-repeat (a held key or a bouncing
 * clicker must not toggle blackout twice).
 */
export function installKeyHandler(
  keymap: () => Record<string, KeyAction>,
  dispatch: (a: Action) => void,
): () => void {
  const onKey = (e: KeyboardEvent) => {
    if (e.repeat || isEditable(e.target)) return;
    const mapped = keymap()[keyName(e)];
    if (!mapped) return;
    // Space/Enter on a focused button would also click it.
    if (e.target instanceof HTMLButtonElement && (e.key === " " || e.key === "Enter")) return;
    const action = toAction(mapped);
    if (!action) return;
    e.preventDefault();
    dispatch(action);
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
