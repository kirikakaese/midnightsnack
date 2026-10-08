<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import { Button, t, type HostConnection } from "@midnightsnack/ui";

  interface Props {
    conn: HostConnection;
  }
  let { conn }: Props = $props();
  const m = $derived(conn.live?.masters);
  const started = $derived(!!conn.live?.program);
</script>

<div class="bar" role="toolbar" aria-label={t("controls.label")}>
  <Button size="xl" onclick={() => conn.action({ action: "prev_cue" })}
    >{t("controls.prev_cue")}</Button
  >
  <Button size="xl" onclick={() => conn.action({ action: "prev" })}>{t("controls.prev")}</Button>
  <Button size="xl" variant="go" class="go" onclick={() => conn.action({ action: "go" })}>
    {started ? t("controls.next") : t("action.go")}
  </Button>
  <Button size="xl" onclick={() => conn.action({ action: "next_cue" })}
    >{t("controls.next_cue")}</Button
  >
  <span class="spacer"></span>
  <Button
    size="xl"
    variant="danger"
    active={m?.blackout}
    onclick={() => conn.action({ action: "toggle_blackout" })}
  >
    {t("controls.blackout")}
  </Button>
  <Button
    size="xl"
    variant="freeze"
    active={m?.freeze}
    onclick={() => conn.action({ action: "toggle_freeze" })}
  >
    {t("controls.freeze")}
  </Button>
  <Button
    size="xl"
    variant="logo"
    active={m?.logo}
    onclick={() => conn.action({ action: "toggle_logo" })}
  >
    {t("controls.logo")}
  </Button>
  <Button size="xl" variant="danger" onclick={() => conn.action({ action: "panic" })}>
    {t("controls.panic")}
  </Button>
</div>

<style>
  .bar {
    display: flex;
    gap: 8px;
    padding: 8px var(--ms-gap);
    background: var(--ms-surface);
    border-top: 1px solid var(--ms-border);
    flex-wrap: wrap;
  }
  .bar :global(.go) {
    min-width: 200px;
  }
  .spacer {
    flex: 1;
  }
</style>
