// SPDX-License-Identifier: GPL-3.0-or-later
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

// The remote is served by the host's embedded server; during development the
// API is proxied to a running host (default port 4747).
const hostUrl = process.env.MIDNIGHTSNACK_HOST ?? "http://127.0.0.1:4747";

export default defineConfig({
  plugins: [svelte()],
  server: {
    port: 5173,
    proxy: {
      "/api": { target: hostUrl, ws: true },
    },
  },
  build: {
    target: "es2020",
    sourcemap: false,
  },
});
