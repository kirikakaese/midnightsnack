// SPDX-License-Identifier: GPL-3.0-or-later
import "@midnightsnack/ui/styles/tokens.css";
import "@midnightsnack/ui/styles/forms.css";
import { preferredLocale, setLocale } from "@midnightsnack/ui";
import { listen } from "@tauri-apps/api/event";
import { mount } from "svelte";
import App from "./App.svelte";
import { host } from "./lib/host";

const target = document.getElementById("app");
if (!target) throw new Error("missing #app");

// Every window (operator, outputs, controllers) uses the host's language setting.
setLocale(preferredLocale(await host.language().catch(() => null)));
listen<string | null>("language-changed", (e) => setLocale(preferredLocale(e.payload))).catch(
  () => {},
);

export default mount(App, { target });
