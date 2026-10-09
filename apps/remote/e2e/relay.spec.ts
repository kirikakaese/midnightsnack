// SPDX-License-Identifier: GPL-3.0-or-later
// The phone remote through a relay (a local relay process stands in for one on the internet).
import { expect, test, type Page } from "@playwright/test";
import { devInfo, Operator } from "./operator";

let op: Operator;

test.beforeEach(async () => {
  op = await Operator.connect(devInfo());
  await op.action({ action: "disconnect_all" });
  await op.action({ action: "set_blackout", on: false });
});

test.afterEach(() => op.close());

async function approveNext(page: Page, name: string, role: "operator" | "presenter") {
  await page.getByLabel("Device name").fill(name);
  await page.getByLabel("PIN shown on the host").fill(op.pin);
  await page.getByRole("button", { name: "Join" }).click();
  await expect(page.getByText("Waiting for the operator")).toBeVisible();
  const pending = await op.nextPending();
  expect(pending.device_name).toBe(name);
  expect(pending.via_relay).toBe(page.url().includes("/r/"));
  await op.action({ action: "approve_pairing", request_id: pending.request_id, role });
}

test("phone pairs and runs the show through the relay", async ({ page }) => {
  expect(op.relayJoinUrl).toContain(`http://127.0.0.1:${devInfo().relay_port}/r/`);
  const sockets: string[] = [];
  page.on("websocket", (ws) => sockets.push(ws.url()));

  await page.goto(op.relayJoinUrl);
  await expect(page.getByText("Connected through the internet relay")).toBeVisible();
  await approveNext(page, "Relay phone", "operator");
  await expect(page.getByText("Operator", { exact: true })).toBeVisible();
  await expect(page.getByText("Relay", { exact: true })).toBeVisible();
  // The join secrets are gone from the address bar.
  expect(page.url()).not.toContain("#");

  await page.getByRole("button", { name: /Welcome deck/ }).click();
  await expect(page.getByText(/Welcome deck · 1 \/ 4/)).toBeVisible();
  // Slide images arrive through the tunnel and are shown from blob URLs.
  await expect(page.locator('img[src^="blob:"]').first()).toBeVisible();

  await page.getByRole("button", { name: "NEXT ▶" }).click();
  await expect.poll(() => op.live?.program?.slide).toBe(1);
  await page.getByRole("button", { name: "BLACKOUT" }).click();
  await expect.poll(() => op.live?.masters.blackout).toBe(true);

  // Every WebSocket went to the relay, none to the host directly.
  expect(sockets.length).toBeGreaterThan(0);
  expect(sockets.every((u) => u.includes("/relay/v1/remote/"))).toBe(true);

  // Still paired after a reload (the relay key is remembered).
  await page.reload();
  await expect(page.getByText(/Welcome deck · 2 \/ 4/)).toBeVisible();
  await page.screenshot({ path: "test-results/remote-relay.png", fullPage: true });
});

test("a LAN phone falls back to the relay and can go back", async ({ page }) => {
  test.setTimeout(60_000);
  const lan = op.joinUrl.replace(/^http:\/\/[^/]+/, `http://127.0.0.1:${devInfo().port}`);
  await page.goto(lan);
  await approveNext(page, "Wandering phone", "presenter");
  await expect(page.getByText("Presenter", { exact: true })).toBeVisible();
  await expect(page.getByText("Relay", { exact: true })).toHaveCount(0);

  // The local network goes away: the host's WebSocket can no longer be reached.
  let lanDown = true;
  await page.routeWebSocket(/\/api\/v1\/ws$/, (ws) => {
    if (lanDown) ws.close();
    else ws.connectToServer();
  });
  await page.reload();
  await expect(page.getByText(/Switching to the internet relay/)).toBeVisible({
    timeout: 20_000,
  });
  await page.waitForURL(/\/r\//, { timeout: 15_000 });
  await expect(page.getByText("Relay", { exact: true })).toBeVisible();
  await expect(page.getByText("Presenter", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "NEXT ▶" })).toBeVisible();

  // Back on the local network: the page moves to the host's LAN address with the token.
  lanDown = false;
  await page.getByRole("button", { name: "Use local network" }).click();
  await page.waitForURL((u) => !u.pathname.startsWith("/r/"));
  expect(page.url()).not.toContain("#");
  await expect(page.getByText("Relay", { exact: true })).toHaveCount(0);
  // Paired there too (no join form). Whether the WebSocket connects depends on the test
  // machine's network: the LAN route is the host's interface address, not 127.0.0.1.
  await expect(page.getByLabel("PIN shown on the host")).toHaveCount(0);
  const token = await page.evaluate(() => localStorage.getItem("midnightsnack.token"));
  expect(token).toBeTruthy();
});

test("the stay button keeps the phone on the local network", async ({ page }) => {
  test.setTimeout(45_000);
  const lan = op.joinUrl.replace(/^http:\/\/[^/]+/, `http://127.0.0.1:${devInfo().port}`);
  await page.goto(lan);
  await approveNext(page, "Loyal phone", "presenter");
  await expect(page.getByText("Presenter", { exact: true })).toBeVisible();
  await page.routeWebSocket(/\/api\/v1\/ws$/, (ws) => ws.close());
  await page.reload();
  await page.getByRole("button", { name: "Stay" }).click({ timeout: 20_000 });
  await page.waitForTimeout(6000);
  expect(new URL(page.url()).pathname.startsWith("/r/")).toBe(false);
});
