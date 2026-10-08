// SPDX-License-Identifier: GPL-3.0-or-later
// Verifies every locale catalog is valid JSON and contains no keys missing from en.json.
// Missing translations are reported as warnings (they fall back to English at runtime).
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const dir = join(import.meta.dirname, "..", "packages", "ui", "src", "locales");
const load = (f) => JSON.parse(readFileSync(join(dir, f), "utf8"));
const en = load("en.json");
const enKeys = new Set(Object.keys(en));
let failed = false;

for (const [key, value] of Object.entries(en)) {
  if (typeof value !== "string" || value.trim() === "") {
    console.error(`en.json: "${key}" must be a non-empty string`);
    failed = true;
  }
}

for (const file of readdirSync(dir).filter((f) => f.endsWith(".json") && f !== "en.json")) {
  const catalog = load(file);
  for (const key of Object.keys(catalog)) {
    if (!enKeys.has(key)) {
      console.error(`${file}: unknown key "${key}" (not in en.json)`);
      failed = true;
    }
  }
  const missing = [...enKeys].filter((k) => !(k in catalog));
  if (missing.length) console.warn(`${file}: ${missing.length} untranslated keys`);
}

if (failed) process.exit(1);
console.log(`i18n: ${enKeys.size} keys OK`);
