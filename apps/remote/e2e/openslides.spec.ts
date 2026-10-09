// SPDX-License-Identifier: GPL-3.0-or-later
// OpenSlides cues drawn natively on a phone, updated live from the (mock) OpenSlides server.
import { expect, test, type Page } from "@playwright/test";
import { devInfo, Operator } from "./operator";

let op: Operator;

test.beforeEach(async () => {
  op = await Operator.connect(devInfo());
  await op.action({ action: "disconnect_all" });
  await op.action({ action: "set_blackout", on: false });
  await op.action({ action: "set_logo", on: false });
});

test.afterEach(() => op.close());

/** Changes data in the mock OpenSlides server, like an OpenSlides operator would. */
async function openslides(changes: Record<string, unknown>) {
  const res = await fetch(`${devInfo().openslides_url}/mock/set`, {
    method: "POST",
    body: JSON.stringify(changes),
  });
  expect(res.ok).toBe(true);
}

async function pairOperator(page: Page) {
  await page.goto(op.joinUrl.replace(/^http:\/\/[^/]+/, `http://127.0.0.1:${devInfo().port}`));
  await page.getByLabel("Device name").fill("OpenSlides phone");
  await page.getByLabel("PIN shown on the host").fill(op.pin);
  await page.getByRole("button", { name: "Join" }).click();
  const pending = await op.nextPending();
  await op.action({ action: "approve_pairing", request_id: pending.request_id, role: "operator" });
  await expect(page.getByText("Operator", { exact: true })).toBeVisible();
}

async function addAndGo(slide: Record<string, unknown>, name: string) {
  await op.action({ action: "add_open_slides", name, slide: slide as never, at_index: null });
  await expect.poll(() => op.show?.cues.some((c) => c.name === name)).toBe(true);
  const cue = op.show?.cues.findLast((c) => c.name === name);
  await op.action({ action: "go_to", position: { cue_id: cue!.id, slide: 0 } });
}

test("motion and list of speakers are drawn natively and follow OpenSlides", async ({ page }) => {
  expect(devInfo().openslides_url).toBeTruthy();
  await pairOperator(page);
  const current = page.locator(".screens figure").first();

  await addAndGo({ kind: "motion", motion_id: 1 }, "E2E motion");
  await expect(current.getByRole("heading", { name: /Climate budget/ }).first()).toBeVisible();
  await expect(
    current.getByText("Submitted by Ada Lovelace, Dr. Grace Hopper").first(),
  ).toBeVisible();
  await expect(current.getByText("A climate budget of 2% of all spending.").first()).toBeVisible();

  // Edited in OpenSlides: the slide follows.
  await openslides({ "motion/1/title": "Climate budget (revised)" });
  await expect(
    current.getByRole("heading", { name: /Climate budget \(revised\)/ }).first(),
  ).toBeVisible();

  await addAndGo({ kind: "speakers", list_id: null }, "E2E speakers");
  await expect(current.getByText("Dr. Grace Hopper").first()).toBeVisible();
  await expect(current.getByText("Pro", { exact: true }).first()).toBeVisible();
  // The next speaker starts.
  await openslides({ "speaker/32/end_time": 1760000200, "speaker/33/begin_time": 1760000201 });
  await expect(current.locator(".current", { hasText: "Alan Turing" }).first()).toBeVisible();
  await page.screenshot({ path: "test-results/remote-openslides.png", fullPage: true });

  // Put things back for other tests.
  await openslides({
    "motion/1/title": "Climate budget",
    "speaker/32/end_time": null,
    "speaker/33/begin_time": null,
  });
});

test("a follow-projector cue changes with the OpenSlides projector", async ({ page }) => {
  await pairOperator(page);
  const current = page.locator(".screens figure").first();
  await addAndGo({ kind: "follow", projector_id: null }, "E2E follow");
  await expect(current.getByRole("heading", { name: /Climate budget/ }).first()).toBeVisible();

  await openslides({
    "projection/41/content_object_id": "meeting/1",
    "projection/41/type": "agenda_item_list",
  });
  await expect(current.getByRole("heading", { name: "Agenda" }).first()).toBeVisible();
  await expect(current.getByText("Welcome and opening").first()).toBeVisible();

  await openslides({ "projection/41/content_object_id": "motion/1", "projection/41/type": null });
  await expect(current.getByRole("heading", { name: /Climate budget/ }).first()).toBeVisible();
});
