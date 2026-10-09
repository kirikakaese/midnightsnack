// SPDX-License-Identifier: GPL-3.0-or-later
import { expect, test, type Page } from "@playwright/test";
import { devInfo, Operator } from "./operator";

let op: Operator;

test.beforeEach(async () => {
  op = await Operator.connect(devInfo());
  await op.action({ action: "disconnect_all" });
  await op.action({ action: "set_blackout", on: false });
  await op.action({ action: "set_logo", on: false });
  await op.action({ action: "set_stage_message", text: null });
  await op.action({ action: "countdown_reset" });
  for (const id of ["host", "clock"]) {
    await op.action({ action: "set_overlay_visible", overlay_id: id, visible: false });
  }
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

test("operator phone runs text cues, overlays, countdown and stage messages", async ({ page }) => {
  await pair(page, "operator");
  await page.getByRole("button", { name: /Anthem/ }).click();
  await expect(page.getByText(/Anthem · 1 \/ 2/)).toBeVisible();
  // Text slides are rendered on the phone itself.
  await expect(page.getByText("Oh midnight snack, so sweet and true").first()).toBeVisible();

  await page.getByRole("button", { name: "Host", exact: true }).click();
  await expect.poll(() => op.live?.overlays_visible).toContain("host");
  await expect(page.getByRole("button", { name: "Host", exact: true })).toHaveAttribute(
    "aria-pressed",
    "true",
  );

  await page.getByRole("button", { name: "Start" }).click();
  await expect.poll(() => op.live?.countdown.elapsed.running_since_ms).not.toBeNull();

  await page.getByLabel("Message to stage").fill("Two minutes left");
  await page.getByRole("button", { name: "Send" }).click();
  await expect.poll(() => op.live?.stage_message).toBe("Two minutes left");
  await page.screenshot({ path: "test-results/remote-operator-live.png", fullPage: true });
});

test("stage viewer shows operator messages and the countdown", async ({ page }) => {
  await pair(page, "stage_viewer");
  await expect(page.getByText("Stage viewer", { exact: true })).toBeVisible();
  await op.action({ action: "countdown_set", duration_ms: 90_000, label: "Q&A" });
  await op.action({ action: "countdown_start" });
  await op.action({ action: "set_stage_message", text: "Wrap up please" });
  await expect(page.getByRole("alert")).toHaveText("Wrap up please");
  await expect(page.getByText("Q&A")).toBeVisible();
  await page.screenshot({ path: "test-results/remote-stage.png", fullPage: true });
});

test("cues follow their output targets and web pages show as placeholders", async ({ page }) => {
  await pair(page, "operator");
  await op.action({
    action: "put_output",
    output: {
      id: "side",
      name: "Side screen",
      feed: "program",
      overlays: false,
      scaling: "fill",
      margin: 5,
    },
  });
  await op.action({
    action: "add_web",
    name: "Agenda page",
    web: {
      url: "https://example.org/agenda",
      zoom: 100,
      block_navigation: true,
      forward_keys: false,
      persist_session: false,
      openslides: false,
    },
    at_index: null,
  });
  const cue = (name: string) => op.show?.cues.find((c) => c.name === name);
  await expect.poll(() => cue("Agenda page")).toBeTruthy();
  const web = cue("Agenda page");
  const deck = cue("Welcome deck");
  try {
    await op.action({ action: "set_cue_targets", cue_id: web!.id, targets: ["main"] });
    await op.action({ action: "go_to", position: { cue_id: deck!.id, slide: 0 } });
    const on = (id: string) => op.live?.outputs.find((o) => o.output_id === id)?.position?.cue_id;
    await expect.poll(() => on("side")).toBe(deck!.id);

    await op.action({ action: "go_to", position: { cue_id: web!.id, slide: 0 } });
    await expect.poll(() => on("main")).toBe(web!.id);
    // The side screen is not a target of the web cue and keeps the deck.
    expect(on("side")).toBe(deck!.id);

    await expect(page.getByText(/Agenda page · 1 \/ 1/)).toBeVisible();
    await expect(page.getByText("example.org").first()).toBeVisible();

    await op.action({ action: "set_test_pattern", pattern: "bars" });
    await expect.poll(() => op.live?.test_pattern).toBe("bars");
    await op.action({ action: "set_test_pattern", pattern: null });
  } finally {
    await op.action({ action: "remove_cue", cue_id: web!.id });
    await op.action({ action: "remove_output", output_id: "side" });
  }
});
