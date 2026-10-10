// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { addMessages, preferredLocale, setLocale, t } from "./index.svelte";

describe("i18n", () => {
  it("translates English keys", () => {
    setLocale("en");
    expect(t("status.connected")).toBe("Connected");
  });

  it("falls back to English for missing keys", () => {
    addMessages("xx", { "status.connected": "Xonnected" });
    setLocale("xx");
    expect(t("status.connected")).toBe("Xonnected");
    expect(t("status.connecting")).toBe("Connecting…");
    setLocale("en");
  });

  it("ships German", () => {
    setLocale("de");
    expect(t("status.connected")).toBe("Verbunden");
    expect(t("relay.remotes", { n: 1 })).toBe("1 Gerät über das Relay");
    setLocale("en");
  });

  it("picks the chosen, then the browser's language", () => {
    expect(preferredLocale("de", ["en-US"])).toBe("de");
    expect(preferredLocale(null, ["fr-FR", "de-AT", "en"])).toBe("de");
    expect(preferredLocale("tlh", ["fr"])).toBe("en");
    expect(preferredLocale(null, [])).toBe("en");
  });

  it("formats ICU arguments", () => {
    addMessages("en", { "test.count": "{n, plural, one {# slide} other {# slides}}" });
    expect(t("test.count" as never, { n: 1 })).toBe("1 slide");
    expect(t("test.count" as never, { n: 3 })).toBe("3 slides");
  });
});
