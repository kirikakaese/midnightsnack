// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { buildKeymap, DEFAULT_KEYMAP, keyName } from "./keymap";

const ev = (key: string, mods: Partial<KeyboardEvent> = {}) => ({
  key,
  shiftKey: false,
  ctrlKey: false,
  altKey: false,
  metaKey: false,
  ...mods,
});

describe("keymap", () => {
  it("normalizes keys", () => {
    expect(keyName(ev("B"))).toBe("b");
    expect(keyName(ev("B", { shiftKey: true }))).toBe("b");
    expect(keyName(ev("F5", { shiftKey: true }))).toBe("Shift+F5");
    expect(keyName(ev("PageDown"))).toBe("PageDown");
    expect(keyName(ev("s", { ctrlKey: true }))).toBe("Ctrl+s");
  });

  it("covers common clicker keys", () => {
    for (const k of ["PageDown", "PageUp", "ArrowRight", "ArrowLeft", "F5", "Escape", ".", "b"]) {
      expect(DEFAULT_KEYMAP[k]).toBeDefined();
    }
  });

  it("applies valid overrides only", () => {
    const map = buildKeymap({ x: "toggle_freeze", b: "none", y: "format_disk" });
    expect(map.x).toBe("toggle_freeze");
    expect(map.b).toBe("none");
    expect(map.y).toBeUndefined();
  });
});
