// SPDX-License-Identifier: GPL-3.0-or-later
import "@midnightsnack/ui/styles/tokens.css";
import { preferredLocale, setLocale } from "@midnightsnack/ui";
import { mount } from "svelte";
import App from "./App.svelte";
import { loadLanguage } from "./lib/storage";

const target = document.getElementById("app");
if (!target) throw new Error("missing #app");

// The browser's language unless one was chosen on the join page.
setLocale(preferredLocale(loadLanguage()));

export default mount(App, { target });
