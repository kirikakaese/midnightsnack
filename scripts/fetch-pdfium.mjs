// SPDX-License-Identifier: GPL-3.0-or-later
// Downloads prebuilt PDFium (https://github.com/bblanchon/pdfium-binaries, BSD-3-Clause /
// Apache-2.0) into apps/host/src-tauri/resources/pdfium for bundling and tests.
//
// Usage: node scripts/fetch-pdfium.mjs [platform...]
//   platform: linux-x64 | linux-arm64 | mac-x64 | mac-arm64 | mac-univ | win-x64 | win-arm64
//   Defaults to the current platform. "mac-univ" merges both macOS builds with lipo.
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// Must match the pdfium-render feature in crates/render (pdfium_7881).
const PDFIUM_BUILD = "7881";
const root = join(import.meta.dirname, "..");
const outDir = join(root, "apps", "host", "src-tauri", "resources", "pdfium");

function currentPlatform() {
  const arch = process.arch === "arm64" ? "arm64" : "x64";
  if (process.platform === "darwin") return `mac-${arch}`;
  if (process.platform === "win32") return `win-${arch}`;
  return `linux-${arch}`;
}

const libName = (p) =>
  p.startsWith("win") ? "pdfium.dll" : p.startsWith("mac") ? "libpdfium.dylib" : "libpdfium.so";
const libPath = (p) => (p.startsWith("win") ? join("bin", "pdfium.dll") : join("lib", libName(p)));

async function download(platform, dest) {
  const url = `https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F${PDFIUM_BUILD}/pdfium-${platform}.tgz`;
  console.log(`fetch-pdfium: ${url}`);
  const res = await fetch(url);
  if (!res.ok) throw new Error(`download failed: ${res.status} ${url}`);
  const tgz = join(dest, `${platform}.tgz`);
  writeFileSync(tgz, Buffer.from(await res.arrayBuffer()));
  const dir = join(dest, platform);
  mkdirSync(dir, { recursive: true });
  execFileSync("tar", ["-xzf", tgz, "-C", dir]);
  return dir;
}

const platforms = process.argv.slice(2);
if (platforms.length === 0) platforms.push(currentPlatform());

mkdirSync(outDir, { recursive: true });
const work = mkdtempSync(join(tmpdir(), "pdfium-"));
try {
  for (const platform of platforms) {
    if (platform === "mac-univ") {
      const arm = await download("mac-arm64", work);
      const x64 = await download("mac-x64", work);
      execFileSync("lipo", [
        "-create",
        join(arm, libPath("mac-arm64")),
        join(x64, libPath("mac-x64")),
        "-output",
        join(outDir, "libpdfium.dylib"),
      ]);
      copyFileSync(join(arm, "LICENSE"), join(outDir, "PDFIUM-LICENSE"));
      continue;
    }
    const dir = await download(platform, work);
    copyFileSync(join(dir, libPath(platform)), join(outDir, libName(platform)));
    if (existsSync(join(dir, "LICENSE"))) {
      copyFileSync(join(dir, "LICENSE"), join(outDir, "PDFIUM-LICENSE"));
    }
  }
  console.log(`fetch-pdfium: installed into ${outDir}`);
} finally {
  rmSync(work, { recursive: true, force: true });
}
