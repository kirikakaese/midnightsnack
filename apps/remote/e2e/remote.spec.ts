// SPDX-License-Identifier: GPL-3.0-or-later
import { expect, test, type Page } from "@playwright/test";
import { devInfo, Operator } from "./operator";

let op: Operator;

test.beforeEach(async () => {
  op = await Operator.connect(devInfo());
  await op.action({ action: "disconnect_all" });
  await op.action({ action: "set_blackout", on: false });
  await op.action({ action: "set_logo", on: false });
  // `disconnect_all` rotates the PIN and join token; the client tracks the new values.
});

test.afterEach(() => op.close());

/** Opens the QR join URL in the phone, enters the PIN, and lets the operator approve. */
async function pair(page: Page, role: "operator" | "presenter" | "stage_viewer") {
  await page.goto(op.joinUrl.replace(/^http:\/\/[^/]+/, `http://127.0.0.1:${devInfo().port}`));
  await page.getByLabel("Device name").fill(`Test ${role}`);
  await page.getByLabel("PIN shown on the host").fill(op.pin);
  await page.getByRole("button", { name: "Join" }).click();
  await expect(page.getByText("Waiting for the operator")).toBeVisible();
  const pending = await op.nextPending();
  expect(pending.device_name).toBe(`Test ${role}`);
  await op.action({ action: "approve_pairing", request_id: pending.request_id, role });
}

test("wrong PIN is rejected", async ({ page }) => {
  await page.goto(op.joinUrl.replace(/^http:\/\/[^/]+/, `http://127.0.0.1:${devInfo().port}`));
  const wrong = op.pin === "000000" ? "111111" : "000000";
  await page.getByLabel("PIN shown on the host").fill(wrong);
  await page.getByRole("button", { name: "Join" }).click();
  await expect(page.getByRole("alert")).toHaveText("Wrong PIN.");
});

test("operator phone pairs and runs the show", async ({ page }) => {
  await pair(page, "operator");
  await expect(page.getByText("Operator", { exact: true })).toBeVisible();

  // Start from the first cue.
  await page.getByRole("button", { name: /Welcome deck/ }).click();
  await expect(page.getByText(/Welcome deck · 1 \/ 4/)).toBeVisible();
  await expect(page.getByText("Speaker notes for slide 1")).toBeVisible();

  await page.getByRole("button", { name: "NEXT ▶" }).click();
  await expect(page.getByText(/Welcome deck · 2 \/ 4/)).toBeVisible();
  await expect.poll(() => op.live?.program?.slide).toBe(1);

  await page.getByRole("button", { name: "BLACKOUT" }).click();
  await expect.poll(() => op.live?.masters.blackout).toBe(true);
  await expect(page.getByRole("button", { name: "BLACKOUT" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );

  // The phone stays paired across reloads.
  await page.reload();
  await expect(page.getByText(/Welcome deck · 2 \/ 4/)).toBeVisible();
  await page.screenshot({ path: "test-results/remote-operator.png", fullPage: true });
});

test("presenter only gets next/prev", async ({ page }) => {
  await pair(page, "presenter");
  await expect(page.getByText("Presenter", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "NEXT ▶" })).toBeVisible();
  await expect(page.getByRole("button", { name: "BLACKOUT" })).toHaveCount(0);
});

test("stage viewer sees a read-only stage display", async ({ page }) => {
  await pair(page, "stage_viewer");
  await expect(page.getByText("Stage viewer", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "NEXT ▶" })).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Notes" })).toBeVisible();
});

test("removed device is told to pair again", async ({ page }) => {
  await pair(page, "presenter");
  await expect(page.getByText("Presenter", { exact: true })).toBeVisible();
  await op.action({ action: "disconnect_all" });
  await expect(page.getByText("This device is not paired")).toBeVisible();
});
