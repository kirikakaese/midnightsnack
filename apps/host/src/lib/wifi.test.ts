// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { randomPassword, wifiQrText } from "./wifi";

describe("wifi", () => {
  it("builds WIFI: QR texts with escaping", () => {
    expect(wifiQrText("midnightsnack", "snacks-at-9")).toBe(
      "WIFI:T:WPA;S:midnightsnack;P:snacks-at-9;;",
    );
    expect(wifiQrText('a;b,c:d"e\\', "pass;word")).toBe(
      String.raw`WIFI:T:WPA;S:a\;b\,c\:d\"e\\;P:pass\;word;;`,
    );
  });

  it("refuses values that are not a WPA2 network", () => {
    expect(wifiQrText("", "longenough")).toBeNull();
    expect(wifiQrText("ok", "short")).toBeNull();
    expect(wifiQrText("x".repeat(33), "longenough")).toBeNull();
  });

  it("generates readable passwords", () => {
    const p = randomPassword();
    expect(p).toMatch(/^[a-km-np-z2-9]{12}$/);
    expect(randomPassword()).not.toBe(p);
  });
});
