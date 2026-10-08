// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { addMessages, setLocale, t } from "./index.svelte";

describe("i18n", () => {
  it("translates English keys", () => {
    setLocale("en");
    expect(t("status.connected")).toBe("Connected");
  });

  it("falls back to English for missing keys", () => {
    addMessages("de", { "status.connected": "Verbunden" });
    setLocale("de");
    expect(t("status.connected")).toBe("Verbunden");
    expect(t("status.connecting")).toBe("Connecting…");
    setLocale("en");
  });

  it("formats ICU arguments", () => {
    addMessages("en", { "test.count": "{n, plural, one {# slide} other {# slides}}" });
    expect(t("test.count" as never, { n: 1 })).toBe("1 slide");
    expect(t("test.count" as never, { n: 3 })).toBe("3 slides");
  });
});
