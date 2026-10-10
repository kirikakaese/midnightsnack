// SPDX-License-Identifier: GPL-3.0-or-later
// Verifies every locale catalog is valid JSON and contains no keys missing from en.json, and
// that every English key is still used by the code. Missing translations are reported as
// warnings (they fall back to English at runtime).
import { execFileSync } from "node:child_process";
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

// Unused keys: not referenced literally, nor by a template like t(`error.${code}`).
const root = join(import.meta.dirname, "..");
const sources = execFileSync(
  "git",
  ["ls-files", "--cached", "--others", "--exclude-standard", "apps", "packages/ui", "integrations"],
  { cwd: root, encoding: "utf8" },
)
  .split("\n")
  .filter((f) => /\.(svelte|ts)$/.test(f) && !/generated|\.test\.ts$|\/e2e\//.test(f))
  .map((f) => readFileSync(join(root, f), "utf8"))
  .join("\n");
const prefixes = [...sources.matchAll(/`([a-z][a-z_.]*[._])\$\{/g)].map((m) => m[1]);
for (const key of enKeys) {
  const quoted = [`"${key}"`, `'${key}'`, `\`${key}\``].some((q) => sources.includes(q));
  if (!quoted && !prefixes.some((p) => key.startsWith(p))) {
    console.error(`en.json: "${key}" is not used anywhere`);
    failed = true;
  }
}

if (failed) process.exit(1);
console.log(`i18n: ${enKeys.size} keys OK`);
