// SPDX-License-Identifier: GPL-3.0-or-later
// A phone set to German shows the remote in German, with no English left over.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test, type Page } from "@playwright/test";
import { devInfo, Operator } from "./operator";

const locales = join(import.meta.dirname, "..", "..", "..", "packages", "ui", "src", "locales");
const en: Record<string, string> = JSON.parse(readFileSync(join(locales, "en.json"), "utf8"));
const de: Record<string, string> = JSON.parse(readFileSync(join(locales, "de.json"), "utf8"));

/**
 * English texts that differ from their German translation, as fixed phrases (the parts between
 * placeholders), long enough not to match names or words both languages share.
 */
const englishOnly = Object.keys(en)
  .filter((k) => en[k] !== de[k])
  .flatMap((k) => en[k]!.split(/\{[^}]*\}|[{}#]/))
  .map((s) => s.trim())
  .filter((s) => s.length >= 12 && !Object.values(de).some((d) => d.includes(s)));

async function expectNoEnglish(page: Page) {
  const text = await page.locator("body").innerText();
  const found = englishOnly.filter((s) => text.includes(s));
  expect(found, "English text on a German page").toEqual([]);
}

test.use({ locale: "de-DE" });

let op: Operator;
test.beforeEach(async () => {
  op = await Operator.connect(devInfo());
  await op.action({ action: "disconnect_all" });
});
test.afterEach(() => op.close());

test("a German phone pairs and runs the show in German", async ({ page }) => {
  await page.goto(op.joinUrl.replace(/^http:\/\/[^/]+/, `http://127.0.0.1:${devInfo().port}`));
  await expect(page).toHaveTitle("DECK-Fernbedienung");
  await expect(page.locator("html")).toHaveAttribute("lang", "de");
  await expectNoEnglish(page);
  await page.getByLabel("Gerätename").fill("Telefon");
  await page.getByLabel("Auf dem Host angezeigte PIN").fill(op.pin);
  await page.getByRole("button", { name: "Beitreten" }).click();
  await expect(page.getByText("Warte darauf, dass der Operator")).toBeVisible();
  await expectNoEnglish(page);
  const pending = await op.nextPending();
  await op.action({ action: "approve_pairing", request_id: pending.request_id, role: "operator" });

  await expect(page.getByRole("button", { name: "Bühnenansicht" })).toBeVisible();
  await expectNoEnglish(page);
  await page.getByRole("button", { name: "Bühnenansicht" }).click();
  await expect(page.getByRole("button", { name: "Steuerung" })).toBeVisible();
  await expectNoEnglish(page);
});

test("a phone can choose its language", async ({ page }) => {
  await page.goto(op.joinUrl.replace(/^http:\/\/[^/]+/, `http://127.0.0.1:${devInfo().port}`));
  await page.getByLabel("Sprache").selectOption("en");
  await expect(page.getByLabel("Device name")).toBeVisible();
  await page.reload();
  await expect(page.getByRole("button", { name: "Join" })).toBeVisible();
  await page.getByLabel("Language").selectOption("");
  await expect(page.getByRole("button", { name: "Beitreten" })).toBeVisible();
});
