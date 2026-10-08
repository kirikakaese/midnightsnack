// SPDX-License-Identifier: GPL-3.0-or-later
// Fails if a tracked source file lacks an SPDX license header in its first lines.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

const extensions = /\.(rs|ts|js|mjs|svelte|css)$/;
const skip = [/^packages\/protocol\/src\/generated\//, /\.d\.ts$/, /(^|\/)dist\//];

const files = execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard"], {
  encoding: "utf8",
})
  .split("\n")
  .filter((f) => extensions.test(f) && !skip.some((re) => re.test(f)));

const missing = files.filter((f) => {
  const head = readFileSync(f, "utf8").split("\n", 5).join("\n");
  return !head.includes("SPDX-License-Identifier: GPL-3.0-or-later");
});

if (missing.length) {
  console.error("Missing SPDX header:\n  " + missing.join("\n  "));
  process.exit(1);
}
console.log(`spdx: ${files.length} files OK`);
