<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import { Button, t } from "@midnightsnack/ui";
  import { host, type DisplayInfo } from "../lib/host";
  import WindowControl from "./WindowControl.svelte";

  let displays = $state<DisplayInfo[]>([]);
  const refresh = async () => (displays = await host.listDisplays());
  $effect(() => {
    refresh();
  });
</script>

<div class="outputs">
  <div class="head">
    <h3>{t("output.title")}</h3>
    <Button variant="ghost" onclick={refresh} aria-label={t("output.refresh")}>⟳</Button>
  </div>
  <WindowControl kind="output" {displays} />
  <h3>{t("window.stage_title")}</h3>
  <p class="hint">{t("window.stage_hint")}</p>
  <WindowControl kind="stage" {displays} />
</div>

<style>
  .outputs {
    display: grid;
    gap: 10px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h3 {
    margin: 4px 0 0;
    font-size: 0.75rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ms-text-muted);
  }
  .hint {
    margin: 0;
    font-size: 0.8rem;
    color: var(--ms-text-muted);
  }
</style>
