<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!--
  Read-only stage display for presenters: what is on screen, what comes next, notes, big
  timers and messages from the operator.
-->
<script lang="ts">
  import { onDestroy } from "svelte";
  import type { HostConnection } from "../client/connection.svelte";
  import { Ticker, elapsedMs, formatClock, formatDuration } from "../client/time.svelte";
  import { t } from "../i18n/index.svelte";
  import Stage from "./Stage.svelte";

  interface Props {
    conn: HostConnection;
    /** `monitor` decodes video previews (host); `thumb` does not (phones). */
    mode?: "monitor" | "thumb";
  }
  let { conn, mode = "thumb" }: Props = $props();

  const ticker = new Ticker(250);
  onDestroy(() => ticker.stop());
  const hostNow = $derived(ticker.now + conn.clockOffset);

  const live = $derived(conn.live);
  const programCue = $derived(conn.cue(live?.program?.cue_id));
  const nextCue = $derived(conn.cue(live?.next?.cue_id));
  const notes = $derived(
    programCue && live?.program ? (programCue.slide_notes[live.program.slide] ?? "") : "",
  );
  const countdown = $derived.by(() => {
    const c = live?.countdown;
    if (!c) return null;
    const running = c.elapsed.running_since_ms !== null || c.elapsed.accumulated_ms > 0;
    const left = c.duration_ms - elapsedMs(c.elapsed, hostNow);
    return { running, left, label: c.label };
  });
</script>

<div class="stage-display">
  {#if live?.stage_message}
    <div class="message" role="alert">{live.stage_message}</div>
  {/if}

  <section class="screens">
    <figure class="current">
      <div class="screen"><Stage {conn} {mode} /></div>
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
      <div class="screen"><Stage {conn} which="next" masters={false} {mode} /></div>
      <figcaption>
        {t("remote.next")}{#if nextCue}&nbsp;· {nextCue.name}{/if}
      </figcaption>
    </figure>
  </section>

  <section class="timers">
    {#if countdown?.running}
      <div
        class="timer big"
        class:over={countdown.left < 0}
        class:warn={countdown.left >= 0 && countdown.left < 60_000}
      >
        <span>{countdown.label || t("countdown.title")}</span>
        <strong>
          {countdown.left < 0 ? "+" : ""}{formatDuration(
            Math.abs(countdown.left) + (countdown.left > 0 ? 999 : 0),
          )}
        </strong>
      </div>
    {/if}
    <div class="timer">
      <span>{t("timer.show")}</span><strong
        >{formatDuration(elapsedMs(live?.show_timer, hostNow))}</strong
      >
    </div>
    <div class="timer">
      <span>{t("timer.slide")}</span><strong
        >{formatDuration(elapsedMs(live?.slide_timer, hostNow))}</strong
      >
    </div>
    <div class="timer"><span>{t("timer.clock")}</span><strong>{formatClock(hostNow)}</strong></div>
  </section>

  <section class="notes">
    <h2>{t("remote.notes")}</h2>
    <p>{notes || t("notes.none")}</p>
  </section>
</div>

<style>
  .stage-display {
    display: grid;
    gap: var(--ms-gap);
    align-content: start;
  }
  .message {
    padding: 14px 18px;
    border-radius: var(--ms-radius);
    background: var(--ms-warn);
    color: #241500;
    font-size: 1.6rem;
    font-weight: 700;
    text-align: center;
    white-space: pre-line;
    animation: pulse 1.6s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      filter: brightness(1.25);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .message {
      animation: none;
    }
  }
  .screens {
    display: grid;
    grid-template-columns: 2fr 1fr;
    gap: 10px;
    align-items: start;
  }
  figure {
    margin: 0;
  }
  .screen {
    aspect-ratio: 16 / 9;
    border: 1px solid var(--ms-border);
    border-radius: var(--ms-radius-sm);
    overflow: hidden;
  }
  figcaption {
    margin-top: 4px;
    color: var(--ms-text-muted);
    font-size: 0.85rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .timers {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 8px;
  }
  .timer {
    display: grid;
    padding: 8px 12px;
    background: var(--ms-surface);
    border: 1px solid var(--ms-border);
    border-radius: var(--ms-radius-sm);
  }
  .timer span {
    font-size: 0.75rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ms-text-muted);
  }
  .timer strong {
    font-family: var(--ms-font-mono);
    font-size: 2.2rem;
    font-variant-numeric: tabular-nums;
  }
  .timer.big {
    grid-column: 1 / -1;
  }
  .timer.big strong {
    font-size: 4.5rem;
    line-height: 1.1;
  }
  .timer.warn strong {
    color: var(--ms-warn);
  }
  .timer.over {
    border-color: var(--ms-danger);
  }
  .timer.over strong {
    color: var(--ms-danger);
  }
  .notes h2 {
    margin: 0 0 6px;
    font-size: 0.8rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ms-text-muted);
  }
  .notes p {
    margin: 0;
    font-size: 1.6rem;
    line-height: 1.45;
    white-space: pre-wrap;
  }
</style>
