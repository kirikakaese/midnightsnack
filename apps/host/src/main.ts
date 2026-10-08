// SPDX-License-Identifier: GPL-3.0-or-later
import "@midnightsnack/ui/styles/tokens.css";
import "@midnightsnack/ui/styles/forms.css";
import { mount } from "svelte";
import App from "./App.svelte";

const target = document.getElementById("app");
if (!target) throw new Error("missing #app");

export default mount(App, { target });
