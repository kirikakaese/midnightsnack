<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { Action, Role } from "@midnightsnack/protocol";
  import {
    Button,
    HostConnection,
    Panel,
    Stage,
    StatusDot,
    Ticker,
    elapsedMs,
    formatDuration,
    t,
  } from "@midnightsnack/ui";
  import { onDestroy, untrack } from "svelte";
  import { storage, wsUrl } from "../lib/storage";
  import { keepScreenOn, tap } from "../lib/wakelock";

  interface Props {
    token: string;
    onunpaired: () => void;
  }
  let { token, onunpaired }: Props = $props();

  // The parent remounts this view (`{#key}`) when the token changes.
  const conn = new HostConnection({ wsUrl: wsUrl(), httpBase: "", token: untrack(() => token) });
  conn.connect();
  const ticker = new Ticker(500);
  const releaseWake = keepScreenOn();
  let view = $state<"control" | "stage">("control");

  onDestroy(() => {
    conn.close();
    ticker.stop();
    releaseWake();
  });

  $effect(() => {
    if (conn.status === "unauthorized") storage.setToken(null);
  });

  const rank: Record<Role, number> = { stage_viewer: 0, presenter: 1, operator: 2, admin: 3 };
  const role = $derived(conn.session?.role ?? "stage_viewer");
  const can = (min: Role) => rank[role] >= rank[min];
  const showStage = $derived(view === "stage" || role === "stage_viewer");

  const live = $derived(conn.live);
  const programCue = $derived(conn.cue(live?.program?.cue_id));
  const nextCue = $derived(conn.cue(live?.next?.cue_id));
  const notes = $derived(
    programCue && live?.program ? (programCue.slide_notes[live.program.slide] ?? "") : "",
  );
  const hostNow = $derived(ticker.now + conn.clockOffset);
  const showTime = $derived(formatDuration(elapsedMs(live?.show_timer, hostNow)));
  const slideTime = $derived(formatDuration(elapsedMs(live?.slide_timer, hostNow)));
  const statusKind = $derived(
    conn.status === "connected" ? "ok" : conn.status === "connecting" ? "pending" : "error",
  );

  function send(action: Action) {
    tap();
    conn.action(action);
  }
</script>

{#if conn.status === "unauthorized"}
  <main class="message">
    <p>{t("remote.unpaired")}</p>
    <p>{t("remote.scan_again")}</p>
    <Button size="lg" onclick={onunpaired}>{t("remote.pair_again")}</Button>
  </main>
{:else if conn.status === "incompatible"}
  <main class="message"><p>{t("remote.incompatible")}</p></main>
{:else}
  <div class="remote" class:stage={showStage}>
    <header>
      <StatusDot status={statusKind} label={conn.host?.name ?? t("status.connecting")} />
      <span class="role">{t(`role.${role}`)}</span>
      {#if role !== "stage_viewer"}
        <button class="switch" onclick={() => (view = view === "stage" ? "control" : "stage")}>
          {view === "stage" ? t("remote.control_view") : t("remote.stage_view")}
        </button>
      {/if}
    </header>

    {#if conn.status !== "connected"}
      <div class="banner" role="status">{t("remote.reconnecting")}</div>
    {/if}

    <section class="screens">
      <figure>
        <div class="screen"><Stage {conn} /></div>
        <figcaption>
          {t("remote.current")}
          {#if programCue && live?.program}
            · {programCue.name} · {t("remote.slide_of", {
              n: live.program.slide + 1,
              total: programCue.slide_count,
            })}
          {/if}
        </figcaption>
      </figure>
      <figure class="next">
        <div class="screen"><Stage {conn} which="next" masters={false} /></div>
        <figcaption>
          {t("remote.next")}{#if nextCue}&nbsp;· {nextCue.name}{/if}
        </figcaption>
      </figure>
    </section>

    <section class="timers" aria-label={t("timer.show")}>
      <div><span>{t("timer.show")}</span><strong>{showTime}</strong></div>
      <div><span>{t("timer.slide")}</span><strong>{slideTime}</strong></div>
    </section>

    {#if notes || showStage}
      <Panel title={t("remote.notes")}>
        <p class="notes">{notes || t("notes.none")}</p>
      </Panel>
    {/if}

    {#if !showStage}
      {#if can("operator")}
        <section class="masters">
          <Button
            size="lg"
            variant="danger"
            active={live?.masters.blackout}
            onclick={() => send({ action: "toggle_blackout" })}
          >
            {t("controls.blackout")}
          </Button>
          <Button
            size="lg"
            variant="freeze"
            active={live?.masters.freeze}
            onclick={() => send({ action: "toggle_freeze" })}
          >
            {t("controls.freeze")}
          </Button>
          <Button
            size="lg"
            variant="logo"
            active={live?.masters.logo}
            onclick={() => send({ action: "toggle_logo" })}
          >
            {t("controls.logo")}
          </Button>
        </section>

        <Panel title={t("remote.cues")}>
          <ol class="cues">
            {#each conn.show?.cues ?? [] as cue (cue.id)}
              <li>
                <button
                  class:live={cue.id === live?.program?.cue_id}
                  disabled={cue.slide_count === 0}
                  onclick={() => send({ action: "go_to", position: { cue_id: cue.id, slide: 0 } })}
                >
                  <span class="tag" style:background={cue.color ?? "transparent"}></span>
                  {cue.name}
                </button>
              </li>
            {/each}
          </ol>
        </Panel>
      {/if}

      {#if can("presenter")}
        <nav class="nav" aria-label={t("controls.label")}>
          <Button size="xl" onclick={() => send({ action: "prev" })}>{t("controls.prev")}</Button>
          <Button size="xl" variant="go" onclick={() => send({ action: "next" })}
            >{t("controls.next")}</Button
          >
        </nav>
      {/if}
    {/if}

    {#if conn.lastError}
      <div class="toast" role="alert">{t(`error.${conn.lastError}`)}</div>
    {/if}
  </div>
{/if}

<style>
  .remote {
    display: grid;
    gap: var(--ms-gap);
    padding: max(10px, env(safe-area-inset-top)) var(--ms-gap)
      calc(150px + env(safe-area-inset-bottom));
    max-width: 900px;
    margin: 0 auto;
  }
  .remote.stage {
    padding-bottom: var(--ms-gap);
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .role {
    margin-left: auto;
    color: var(--ms-text-muted);
    font-size: 0.85rem;
  }
  .switch {
    font: inherit;
    font-size: 0.85rem;
    padding: 6px 10px;
    background: var(--ms-surface-2);
    color: var(--ms-text);
    border: 1px solid var(--ms-border);
    border-radius: var(--ms-radius-sm);
  }
  .banner {
    padding: 8px 12px;
    background: var(--ms-warn);
    color: #241500;
    border-radius: var(--ms-radius-sm);
    font-weight: 600;
  }
  .screens {
    display: grid;
    grid-template-columns: 2fr 1fr;
    gap: 8px;
    align-items: start;
  }
  figure {
    margin: 0;
  }
  .screen {
    aspect-ratio: 16 / 9;
    border-radius: var(--ms-radius-sm);
    overflow: hidden;
    border: 1px solid var(--ms-border);
  }
  figcaption {
    margin-top: 4px;
    font-size: 0.8rem;
    color: var(--ms-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .timers {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .timers div {
    display: grid;
    padding: 8px 12px;
    background: var(--ms-surface);
    border: 1px solid var(--ms-border);
    border-radius: var(--ms-radius-sm);
  }
  .timers span {
    font-size: 0.75rem;
    color: var(--ms-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .timers strong {
    font-family: var(--ms-font-mono);
    font-size: 1.6rem;
    font-variant-numeric: tabular-nums;
  }
  .stage .timers strong {
    font-size: 3rem;
  }
  .notes {
    margin: 0;
    font-size: 1.15rem;
    line-height: 1.45;
    white-space: pre-wrap;
  }
  .stage .notes {
    font-size: 1.5rem;
  }
  .masters {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }
  .cues {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
  }
  .cues button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 44px;
    padding: 6px 10px;
    font: inherit;
    text-align: left;
    background: var(--ms-surface-2);
    color: var(--ms-text);
    border: 2px solid transparent;
    border-radius: var(--ms-radius-sm);
  }
  .cues button.live {
    border-color: var(--ms-go);
  }
  .tag {
    width: 8px;
    height: 24px;
    border-radius: 3px;
  }
  .nav {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    display: grid;
    grid-template-columns: 1fr 2fr;
    gap: 8px;
    padding: 10px var(--ms-gap) calc(10px + env(safe-area-inset-bottom));
    background: var(--ms-surface);
    border-top: 1px solid var(--ms-border);
  }
  .nav :global(.ms-btn) {
    min-height: 110px;
  }
  .message {
    display: grid;
    gap: 12px;
    padding: 32px var(--ms-gap);
    text-align: center;
    font-size: 1.1rem;
  }
  .toast {
    position: fixed;
    left: 50%;
    bottom: 150px;
    transform: translateX(-50%);
    padding: 10px 16px;
    background: var(--ms-danger);
    color: #fff;
    border-radius: var(--ms-radius);
    font-weight: 600;
    max-width: 90vw;
  }
  @media (orientation: landscape) and (max-height: 500px) {
    .screens {
      grid-template-columns: 1fr 1fr;
    }
  }
</style>
