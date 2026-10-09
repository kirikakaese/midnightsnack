<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Play/pause, restart and seek for the media cue on the output. -->
<script lang="ts">
  import {
    Button,
    formatDuration,
    mediaPosition,
    t,
    type HostConnection,
    type Ticker,
  } from "@midnightsnack/ui";

  interface Props {
    conn: HostConnection;
    ticker: Ticker;
  }
  let { conn, ticker }: Props = $props();

  const media = $derived(conn.live?.media ?? null);
  const cue = $derived(conn.cue(media?.cue_id));
  const info = $derived(cue?.media ?? null);
  const playing = $derived(!!media && media.position.running_since_ms !== null && !media.ended);
  const position = $derived(
    media && info
      ? mediaPosition(media.position, info.options, info.duration_ms, ticker.now + conn.clockOffset)
      : 0,
  );
  const end = $derived(info ? (info.options.end_ms ?? info.duration_ms) : null);
</script>

{#if media && cue && info}
  <div class="transport" role="group" aria-label={t("media.transport")}>
    {#if playing}
      <Button onclick={() => conn.action({ action: "media_pause" })}>{t("media.pause")}</Button>
    {:else}
      <Button variant="go" onclick={() => conn.action({ action: "media_play" })}
        >{t("media.play")}</Button
      >
    {/if}
    <Button onclick={() => conn.action({ action: "media_restart" })}>{t("media.restart")}</Button>
    <span class="time">{formatDuration(position)}</span>
    <input
      type="range"
      min={info.options.start_ms}
      max={end ?? Math.max(position, 1)}
      step="100"
      value={position}
      disabled={end === null}
      aria-label={t("media.seek")}
      onchange={(e) =>
        conn.action({
          action: "media_seek",
          position_ms: Math.round(Number(e.currentTarget.value)),
        })}
    />
    <span class="time">{end !== null ? formatDuration(end) : "–:––"}</span>
    {#if info.options.loop}<span class="tag">{t("media.looping")}</span>{/if}
  </div>
{/if}

<style>
  .transport {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 6px;
  }
  input[type="range"] {
    flex: 1;
    accent-color: var(--ms-accent);
  }
  .time {
    font-family: var(--ms-font-mono);
    font-variant-numeric: tabular-nums;
    color: var(--ms-text-muted);
  }
  .tag {
    font-size: 0.75rem;
    padding: 2px 6px;
    border-radius: 4px;
    background: var(--ms-surface-3);
  }
</style>
