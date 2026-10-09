<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { Overlay } from "@midnightsnack/protocol";
  import type { HostConnection } from "../client/connection.svelte";
  import { elapsedMs, formatClock, formatDuration } from "../client/time.svelte";

  interface Props {
    conn: HostConnection;
    hostNow: number;
  }
  let { conn, hostNow }: Props = $props();

  const visible = $derived(
    (conn.show?.overlays ?? []).filter((o) => conn.live?.overlays_visible.includes(o.id)),
  );
  const countdownLeft = $derived.by(() => {
    const c = conn.live?.countdown;
    return c ? c.duration_ms - elapsedMs(c.elapsed, hostNow) : 0;
  });

  function style(o: Overlay): string {
    return `--fg:${o.color};--bg:${o.background};--scale:${o.scale / 100}`;
  }
</script>

<div class="overlays" aria-hidden="true">
  {#each visible as o (o.id)}
    {#if o.kind.type === "ticker"}
      {@const t = o.kind}
      <div class="ticker {o.position.startsWith('top') ? 'at-top' : 'at-bottom'}" style={style(o)}>
        <!-- Duration: one screen width per (100 / speed) seconds, plus the text length. -->
        <div
          class="track"
          style:animation-duration="{Math.max(4, (100 / t.speed) * (1 + t.text.length / 60))}s"
        >
          {t.text}
        </div>
      </div>
    {:else}
      <div class="box pos-{o.position}" style={style(o)}>
        {#if o.kind.type === "lower_third"}
          <div class="lower-third">
            <div class="title">{o.kind.title}</div>
            {#if o.kind.subtitle}<div class="subtitle">{o.kind.subtitle}</div>{/if}
          </div>
        {:else if o.kind.type === "logo_bug"}
          {@const src = conn.assetUrl(o.kind.image)}
          {#if src}<img class="bug" {src} alt="" />{/if}
        {:else if o.kind.type === "clock"}
          <div class="pill">{formatClock(hostNow).slice(0, o.kind.seconds ? undefined : 5)}</div>
        {:else if o.kind.type === "countdown"}
          <div class="pill countdown" class:over={countdownLeft < 0}>
            {#if conn.live?.countdown.label}<span class="cd-label">{conn.live.countdown.label}</span
              >{/if}
            {countdownLeft < 0 ? "+" : ""}{formatDuration(
              Math.abs(countdownLeft) + (countdownLeft > 0 ? 999 : 0),
            )}
          </div>
        {/if}
      </div>
    {/if}
  {/each}
</div>

<style>
  .overlays {
    position: absolute;
    inset: 0;
    pointer-events: none;
    container-type: size;
    font-family: var(--ms-font, system-ui, sans-serif);
  }
  .box {
    position: absolute;
    font-size: calc(4.2cqh * var(--scale));
    color: var(--fg);
  }
  .pos-top_left {
    top: 5cqh;
    left: 4cqw;
  }
  .pos-top_center {
    top: 5cqh;
    left: 50%;
    transform: translateX(-50%);
  }
  .pos-top_right {
    top: 5cqh;
    right: 4cqw;
  }
  .pos-bottom_left {
    bottom: 7cqh;
    left: 4cqw;
  }
  .pos-bottom_center {
    bottom: 7cqh;
    left: 50%;
    transform: translateX(-50%);
  }
  .pos-bottom_right {
    bottom: 7cqh;
    right: 4cqw;
  }
  .lower-third {
    background: var(--bg);
    padding: 0.5em 1em 0.55em;
    border-left: 0.3em solid var(--fg);
    border-radius: 0.2em;
    min-width: 30cqw;
  }
  .title {
    font-weight: 700;
    font-size: 1.15em;
  }
  .subtitle {
    font-size: 0.8em;
    opacity: 0.85;
  }
  .bug {
    height: calc(9cqh * var(--scale));
    width: auto;
    display: block;
  }
  .pill {
    background: var(--bg);
    padding: 0.25em 0.6em;
    border-radius: 0.3em;
    font-variant-numeric: tabular-nums;
    font-weight: 700;
  }
  .countdown.over {
    color: #ef4444;
  }
  .cd-label {
    font-weight: 500;
    margin-right: 0.5em;
    opacity: 0.8;
  }
  .ticker {
    position: absolute;
    left: 0;
    right: 0;
    height: calc(7cqh * var(--scale));
    background: var(--bg);
    color: var(--fg);
    font-size: calc(4cqh * var(--scale));
    overflow: hidden;
    display: flex;
    align-items: center;
  }
  .ticker.at-bottom {
    bottom: 0;
  }
  .ticker.at-top {
    top: 0;
  }
  .track {
    white-space: nowrap;
    padding-left: 100%;
    animation-name: scroll;
    animation-timing-function: linear;
    animation-iteration-count: infinite;
    will-change: transform;
  }
  @keyframes scroll {
    from {
      transform: translateX(0);
    }
    to {
      transform: translateX(-100%);
    }
  }
</style>
