<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import { Button, Panel, t } from "@midnightsnack/ui";
  import { invoke } from "@tauri-apps/api/core";
  import type { HostInfo } from "@midnightsnack/protocol";

  let info = $state<HostInfo | null>(null);

  $effect(() => {
    invoke<HostInfo>("host_info")
      .then((i) => {
        info = i;
        return invoke("ui_ready");
      })
      .catch(() => (info = null));
  });
</script>

<main class="operator">
  <header>
    <h1>{t("app.name")}</h1>
    {#if info}<span class="version">v{info.app_version}</span>{/if}
  </header>
  <Panel title={t("app.tagline")}>
    <p>{t("operator.placeholder")}</p>
    <Button variant="go" size="lg">{t("action.go")}</Button>
  </Panel>
</main>

<style>
  .operator {
    display: grid;
    grid-template-rows: auto 1fr;
    gap: var(--ms-gap);
    height: 100%;
    padding: var(--ms-gap);
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 12px;
  }
  h1 {
    margin: 0;
    font-size: 1.2rem;
  }
  .version {
    color: var(--ms-text-muted);
    font-family: var(--ms-font-mono);
  }
</style>
