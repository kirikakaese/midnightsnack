<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import {
    Button,
    Panel,
    PointerPad,
    Stage,
    Tabs,
    Ticker,
    elapsedMs,
    formatDuration,
    t,
    type HostConnection,
  } from "@midnightsnack/ui";
  import { onDestroy } from "svelte";
  import { connectToHost, host } from "../lib/host";
  import { buildKeymap, installKeyHandler, type KeyAction } from "../lib/keymap";
  import ConnectPanel from "../operator/ConnectPanel.svelte";
  import CueList from "../operator/CueList.svelte";
  import Inspector from "../operator/Inspector.svelte";
  import LivePanel from "../operator/LivePanel.svelte";
  import MediaTransport from "../operator/MediaTransport.svelte";
  import ShowPanel from "../operator/ShowPanel.svelte";
  import MasterBar from "../operator/MasterBar.svelte";
  import OutputPanel from "../operator/OutputPanel.svelte";
  import SlideStrip from "../operator/SlideStrip.svelte";
  import TopBar from "../operator/TopBar.svelte";

  let conn = $state<HostConnection | null>(null);
  let keymap = $state<Record<string, KeyAction>>(buildKeymap({}));
  let readySent = false;
  let selected = $state<string | null>(null);
  type TabId = "live" | "cue" | "show" | "outputs" | "connect";
  let tab = $state<TabId>("live");
  const tabs = $derived([
    { id: "live" as const, label: t("tab.live") },
    { id: "cue" as const, label: t("tab.cue") },
    { id: "show" as const, label: t("tab.show") },
    { id: "outputs" as const, label: t("tab.outputs") },
    { id: "connect" as const, label: t("tab.connect"), badge: conn?.pending.length ?? 0 },
  ]);
  // Selecting a cue opens the inspector.
  $effect(() => {
    if (selected) tab = "cue";
  });
  // Pairing requests need attention.
  $effect(() => {
    if ((conn?.pending.length ?? 0) > 0) tab = "connect";
  });
  const ticker = new Ticker(250);

  connectToHost().then((c) => (conn = c));
  host.keymap().then((k) => (keymap = buildKeymap(k)));
  const removeKeys = installKeyHandler(
    () => keymap,
    (a) => conn?.action(a),
  );

  $effect(() => {
    if (conn?.status === "connected" && !readySent) {
      readySent = true;
      host.uiReady();
    }
  });

  onDestroy(() => {
    removeKeys();
    ticker.stop();
    conn?.close();
  });

  const programCue = $derived(conn?.cue(conn.live?.program?.cue_id));
  const outputPos = $derived(conn?.live?.output ?? null);
  const outputCue = $derived(conn?.cue(outputPos?.cue_id));
  const nextPos = $derived(conn?.live?.next ?? null);
  const nextCue = $derived(conn?.cue(nextPos?.cue_id));
  const slideNotes = $derived(
    programCue && conn?.live?.program
      ? (programCue.slide_notes[conn.live.program.slide] ?? "")
      : "",
  );
  const slideTime = $derived(
    conn ? elapsedMs(conn.live?.slide_timer, ticker.now + conn.clockOffset) : 0,
  );
  const frozen = $derived(!!conn?.live?.masters.freeze);
  // Point and draw on the audience screen with the mouse on the program monitor.
  let pointerMode = $state<"off" | "point" | "draw">("off");
  const togglePointer = (mode: "point" | "draw") =>
    (pointerMode = pointerMode === mode ? "off" : mode);
</script>

{#if conn}
  <div class="operator">
    <TopBar {conn} {ticker} />

    <aside class="left">
      <CueList {conn} bind:selected />
    </aside>

    <main class="center">
      <div class="monitors">
        <section class="monitor" aria-label={t("monitor.program")}>
          <header>
            <span class="badge program">{t("monitor.program")}</span>
            {#if frozen}<span class="badge freeze">{t("monitor.frozen")}</span>{/if}
            {#if conn.live?.masters.blackout}<span class="badge blackout"
                >{t("controls.blackout")}</span
              >{/if}
            {#if conn.live?.masters.logo}<span class="badge logo">{t("controls.logo")}</span>{/if}
            <!-- The big monitor always shows what the audience sees. -->
            <span class="where">
              {#if outputCue && outputPos}
                {outputCue.name} · {outputPos.slide + 1}/{outputCue.slide_count}
              {:else}
                {t("monitor.nothing_live")}
              {/if}
            </span>
            <span class="pointer-tools" role="group" aria-label={t("pointer.title")}>
              <Button
                variant="ghost"
                active={pointerMode === "point"}
                aria-pressed={pointerMode === "point"}
                onclick={() => togglePointer("point")}>{t("pointer.laser")}</Button
              >
              <Button
                variant="ghost"
                active={pointerMode === "draw"}
                aria-pressed={pointerMode === "draw"}
                onclick={() => togglePointer("draw")}>{t("pointer.draw")}</Button
              >
              {#if conn.live?.drawing}
                <Button variant="ghost" onclick={() => conn?.action({ action: "clear_drawing" })}
                  >{t("pointer.clear")}</Button
                >
              {/if}
            </span>
          </header>
          <div class="screen">
            <PointerPad {conn} mode={pointerMode} color="#ff3b30"><Stage {conn} /></PointerPad>
          </div>
          <MediaTransport {conn} {ticker} />
          {#if frozen}
            <div class="behind">
              <div class="screen small">
                <Stage {conn} which="program" masters={false} />
              </div>
              <span class="where">
                {t("monitor.behind_freeze")}:
                {#if programCue && conn.live?.program}
                  {programCue.name} · {conn.live.program.slide + 1}/{programCue.slide_count}
                {:else}
                  {t("monitor.nothing_live")}
                {/if}
              </span>
            </div>
          {/if}
        </section>
        <section class="monitor" aria-label={t("monitor.preview")}>
          <header>
            <span class="badge preview">{t("monitor.preview")}</span>
            <span class="where">
              {#if nextCue && nextPos}
                {nextCue.name} · {nextPos.slide + 1}/{nextCue.slide_count}
              {:else}
                {t("monitor.end_of_show")}
              {/if}
            </span>
          </header>
          <div class="screen"><Stage {conn} which="next" masters={false} /></div>
        </section>
      </div>

      <div class="lower">
        <Panel title={t("notes.title")}>
          {#snippet actions()}
            <span class="slide-time" aria-label={t("timer.slide")}>
              {t("timer.slide")}: {formatDuration(slideTime)}
            </span>
          {/snippet}
          <div class="notes" aria-live="polite">
            {#if slideNotes}
              <p>{slideNotes}</p>
            {:else}
              <p class="muted">{t("notes.none")}</p>
            {/if}
            {#if programCue?.notes}<p class="cue-notes">{programCue.notes}</p>{/if}
          </div>
        </Panel>
        <SlideStrip {conn} />
      </div>
    </main>

    <aside class="right">
      <Tabs {tabs} bind:active={tab} label={t("tab.label")}>
        {#if tab === "live"}
          <LivePanel {conn} {ticker} />
        {:else if tab === "cue"}
          <Inspector {conn} cueId={selected} />
        {:else if tab === "show"}
          <ShowPanel {conn} />
        {:else if tab === "outputs"}
          <OutputPanel {conn} />
        {:else}
          <ConnectPanel {conn} />
        {/if}
      </Tabs>
    </aside>

    <MasterBar {conn} />

    {#if conn.lastError}
      <div class="toast" role="alert">{t(`error.${conn.lastError}`)}</div>
    {/if}
  </div>
{/if}

<style>
  .operator {
    display: grid;
    grid-template-columns: minmax(260px, 22%) 1fr minmax(280px, 24%);
    grid-template-rows: auto 1fr auto;
    grid-template-areas:
      "top top top"
      "left center right"
      "bar bar bar";
    height: 100vh;
  }
  .operator > :global(header.topbar) {
    grid-area: top;
  }
  .operator > :global(.bar) {
    grid-area: bar;
  }
  .left {
    grid-area: left;
    display: grid;
    min-height: 0;
    padding: var(--ms-gap) 0 var(--ms-gap) var(--ms-gap);
  }
  .right {
    grid-area: right;
    display: grid;
    min-height: 0;
    padding: var(--ms-gap) var(--ms-gap) var(--ms-gap) 0;
  }
  .center {
    grid-area: center;
    display: grid;
    grid-template-rows: auto 1fr;
    gap: var(--ms-gap);
    min-height: 0;
    padding: var(--ms-gap);
  }
  .monitors {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--ms-gap);
  }
  .monitor header {
    display: flex;
    gap: 6px;
    align-items: center;
    margin-bottom: 6px;
    min-height: 26px;
  }
  .pointer-tools {
    display: flex;
    gap: 2px;
    margin-left: auto;
  }
  .where {
    color: var(--ms-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .badge.program {
    background: var(--ms-go);
    color: var(--ms-go-text);
  }
  .badge.preview {
    background: var(--ms-accent);
    color: #fff;
  }
  .badge.freeze {
    background: var(--ms-freeze);
    color: #04202c;
  }
  .badge.blackout {
    background: var(--ms-danger);
    color: #fff;
  }
  .badge.logo {
    background: var(--ms-logo);
    color: #1b0f3a;
  }
  .screen {
    aspect-ratio: 16 / 9;
    border: 1px solid var(--ms-border);
    border-radius: var(--ms-radius-sm);
    overflow: hidden;
  }
  .behind {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 6px;
  }
  .screen.small {
    width: 35%;
    flex: none;
  }
  .lower {
    display: grid;
    grid-template-rows: 1fr auto;
    gap: var(--ms-gap);
    min-height: 0;
  }
  .notes p {
    margin: 0 0 8px;
    font-size: 1.25rem;
    line-height: 1.45;
    white-space: pre-wrap;
  }
  .notes .cue-notes {
    font-size: 1rem;
    color: var(--ms-text-muted);
  }
  .muted {
    color: var(--ms-text-muted);
  }
  .slide-time {
    font-family: var(--ms-font-mono);
    color: var(--ms-text-muted);
  }
  .toast {
    position: fixed;
    bottom: 120px;
    left: 50%;
    transform: translateX(-50%);
    padding: 12px 18px;
    background: var(--ms-danger);
    color: #fff;
    border-radius: var(--ms-radius);
    font-weight: 600;
  }
</style>
