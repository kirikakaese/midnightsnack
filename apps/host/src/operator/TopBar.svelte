<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import {
    Button,
    StatusDot,
    elapsedMs,
    formatClock,
    formatDuration,
    t,
    type HostConnection,
    type Ticker,
  } from "@midnightsnack/ui";
  import { ask, open, save } from "@tauri-apps/plugin-dialog";
  import { isController } from "../lib/mode";

  interface Props {
    conn: HostConnection;
    ticker: Ticker;
  }
  let { conn, ticker }: Props = $props();

  const hostNow = $derived(ticker.now + conn.clockOffset);
  const showTimer = $derived(elapsedMs(conn.live?.show_timer, hostNow));
  const running = $derived(conn.live?.show_timer.running_since_ms != null);
  // Editable copy of the title; resets whenever the host's title changes.
  let title = $derived(conn.show?.title ?? "");

  const showFilter = [{ name: t("file.filter_show"), extensions: ["msnack"] }];

  async function confirmDiscard(): Promise<boolean> {
    if (!conn.show?.dirty) return true;
    return ask(t("file.discard_question"), { title: t("file.discard_title"), kind: "warning" });
  }

  async function newShow() {
    if (await confirmDiscard()) conn.action({ action: "new_show" });
  }

  async function openShow() {
    if (!(await confirmDiscard())) return;
    const path = await open({ multiple: false, filters: showFilter });
    if (typeof path === "string") conn.action({ action: "open_show", path });
  }

  async function saveAs(embed_media: boolean) {
    const path = await save({ filters: showFilter, defaultPath: `${title || "show"}.msnack` });
    if (path) conn.action({ action: "save_show", path, embed_media });
  }

  async function saveShow() {
    if (conn.show?.path) conn.action({ action: "save_show", path: null, embed_media: true });
    else await saveAs(true);
  }

  function rename() {
    if (title !== conn.show?.title) conn.action({ action: "rename_show", title });
  }

  const statusKind = $derived(
    conn.status === "connected" ? "ok" : conn.status === "connecting" ? "pending" : "error",
  );
</script>

<header class="topbar">
  <div class="file" role="group" aria-label={t("file.menu")}>
    <Button size="md" onclick={newShow}>{t("file.new")}</Button>
    {#if !isController()}
      <Button size="md" onclick={openShow}>{t("file.open")}</Button>
    {/if}
    <Button size="md" disabled={isController() && !conn.show?.path} onclick={saveShow}
      >{t("file.save")}</Button
    >
    {#if !isController()}
      <Button size="md" variant="ghost" onclick={() => saveAs(true)}>{t("file.save_as")}</Button>
      <Button size="md" variant="ghost" onclick={() => saveAs(false)}
        >{t("file.save_linked")}</Button
      >
    {/if}
  </div>

  <label class="title">
    <span class="ms-visually-hidden">{t("show.title")}</span>
    <input
      bind:value={title}
      placeholder={t("show.untitled")}
      onblur={rename}
      onkeydown={(e) => e.key === "Enter" && (e.currentTarget as HTMLInputElement).blur()}
    />
    {#if conn.show?.dirty}<span class="dirty" title={t("show.unsaved")}>●</span>{/if}
  </label>

  {#if conn.renderQueued > 0}
    <div
      class="render"
      role="progressbar"
      aria-label={t("render.progress", { n: conn.renderQueued })}
    >
      {t("render.progress", { n: conn.renderQueued })}
    </div>
  {/if}

  <div class="timers">
    <div class="timer" aria-label={t("timer.show")}>
      <span class="label">{t("timer.show")}</span>
      <span class="value" class:running>{formatDuration(showTimer)}</span>
    </div>
    <div class="timer-buttons">
      {#if running}
        <Button size="md" variant="ghost" onclick={() => conn.action({ action: "timer_pause" })}>
          {t("timer.pause")}
        </Button>
      {:else}
        <Button size="md" variant="ghost" onclick={() => conn.action({ action: "timer_start" })}>
          {t("timer.start")}
        </Button>
      {/if}
      <Button size="md" variant="ghost" onclick={() => conn.action({ action: "timer_reset" })}>
        {t("timer.reset")}
      </Button>
    </div>
    <div class="timer" aria-label={t("timer.clock")}>
      <span class="label">{t("timer.clock")}</span>
      <span class="value">{formatClock(ticker.now)}</span>
    </div>
  </div>

  <StatusDot status={statusKind} label={t(`status.${conn.status}`)} />
</header>

<style>
  .topbar {
    display: flex;
    align-items: center;
    gap: var(--ms-gap);
    padding: 8px var(--ms-gap);
    background: var(--ms-surface);
    border-bottom: 1px solid var(--ms-border);
    flex-wrap: wrap;
  }
  .file {
    display: flex;
    gap: 6px;
  }
  .title {
    flex: 1;
    min-width: 160px;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .title input {
    width: 100%;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--ms-radius-sm);
    color: var(--ms-text);
    font: inherit;
    font-size: 1.1rem;
    font-weight: 700;
    padding: 6px 8px;
  }
  .title input:hover,
  .title input:focus {
    border-color: var(--ms-border);
  }
  .dirty {
    color: var(--ms-warn);
  }
  .render {
    font-size: 0.85rem;
    color: var(--ms-text-muted);
  }
  .timers {
    display: flex;
    align-items: center;
    gap: var(--ms-gap);
  }
  .timer {
    display: grid;
    text-align: right;
  }
  .label {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ms-text-muted);
  }
  .value {
    font-family: var(--ms-font-mono);
    font-size: 1.3rem;
    font-variant-numeric: tabular-nums;
  }
  .value.running {
    color: var(--ms-go);
  }
  .timer-buttons {
    display: flex;
    gap: 4px;
  }
</style>
