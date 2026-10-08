<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!--
  Plays a video/audio cue in sync with the host's playback timeline.

  - `output`: plays with sound and reports duration and end of playback to the host.
  - `monitor`: muted mirror (operator preview).
  - `thumb`: no decoding; an icon with a progress bar (phones).

  The element is only revealed once it can show a frame; until then the previous layer stays.
-->
<script lang="ts">
  import type { MediaOptions, MediaPlayback } from "@midnightsnack/protocol";
  import type { HostConnection } from "../client/connection.svelte";
  import { formatDuration } from "../client/time.svelte";
  import { mediaPosition } from "./content";

  interface Props {
    conn: HostConnection;
    cueId: string;
    video: boolean;
    src: string;
    name: string;
    options: MediaOptions;
    durationMs: number | null;
    /** Timeline from the host; `null` shows the first frame, paused (previews). */
    playback: MediaPlayback | null;
    mode: "output" | "monitor" | "thumb";
    hostNow: number;
    onready: () => void;
  }
  let {
    conn,
    cueId,
    video,
    src,
    name,
    options,
    durationMs,
    playback,
    mode,
    hostNow,
    onready,
  }: Props = $props();

  /** Maximum drift before we seek (ms). */
  const DRIFT_MS = 300;

  let el = $state<HTMLMediaElement | null>(null);
  let endReported = false;
  let loadReported = false;

  const desiredMs = $derived(
    playback ? mediaPosition(playback.position, options, durationMs, hostNow) : options.start_ms,
  );
  const playing = $derived(
    !!playback && playback.position.running_since_ms !== null && !playback.ended,
  );
  const endMs = $derived(options.end_ms ?? durationMs);
  const progress = $derived(endMs ? Math.min(1, desiredMs / endMs) : 0);

  $effect(() => {
    if (mode === "thumb") onready();
  });

  // Keep the element on the host's timeline.
  $effect(() => {
    const m = el;
    if (!m || m.readyState < 1) return;
    m.volume = mode === "output" ? Math.max(0, Math.min(1, options.volume)) : 0;
    const drift = Math.abs(m.currentTime * 1000 - desiredMs);
    if (drift > DRIFT_MS) m.currentTime = desiredMs / 1000;
    if (playing && m.paused) m.play().catch(() => {});
    if (!playing && !m.paused) m.pause();
  });

  function onLoaded() {
    const m = el;
    if (!m) return;
    m.currentTime = desiredMs / 1000;
    if (mode === "output" && !loadReported && Number.isFinite(m.duration)) {
      loadReported = true;
      void conn.action({
        action: "media_loaded",
        cue_id: cueId,
        duration_ms: Math.round(m.duration * 1000),
      });
    }
  }

  function onTime() {
    const m = el;
    if (!m || endMs === null) return;
    // Trimmed end: emulate looping or ending.
    if (m.currentTime * 1000 >= endMs - 20) {
      if (options.loop) m.currentTime = options.start_ms / 1000;
      else finish();
    }
  }

  function finish() {
    el?.pause();
    if (mode === "output" && !endReported && playback) {
      endReported = true;
      void conn.action({ action: "media_ended", cue_id: cueId });
    }
  }

  // A new timeline (restart/seek) may end again.
  $effect(() => {
    if (playback && !playback.ended) endReported = false;
  });
</script>

{#if mode === "thumb"}
  <div class="thumb">
    <span class="icon" aria-hidden="true">{video ? "▶" : "♪"}</span>
    <span class="name">{name}</span>
    <span class="time">{formatDuration(desiredMs)}{endMs ? ` / ${formatDuration(endMs)}` : ""}</span
    >
    <div class="bar"><div class="fill" style:width="{progress * 100}%"></div></div>
  </div>
{:else if video}
  <video
    bind:this={el}
    {src}
    preload="auto"
    playsinline
    disablepictureinpicture
    muted={mode !== "output"}
    loop={options.loop && options.end_ms === null}
    onloadedmetadata={onLoaded}
    onloadeddata={onready}
    ontimeupdate={onTime}
    onended={finish}
  ></video>
{:else}
  <audio
    bind:this={el}
    {src}
    preload="auto"
    muted={mode !== "output"}
    loop={options.loop && options.end_ms === null}
    onloadedmetadata={onLoaded}
    oncanplay={onready}
    ontimeupdate={onTime}
    onended={finish}
  ></audio>
  {#if mode === "monitor"}
    <div class="thumb">
      <span class="icon" aria-hidden="true">♪</span>
      <span class="name">{name}</span>
      <div class="bar"><div class="fill" style:width="{progress * 100}%"></div></div>
    </div>
  {/if}
{/if}

<style>
  video {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    background: #000;
  }
  .thumb {
    position: absolute;
    inset: 0;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 4%;
    background: #10131b;
    color: #eef1f7;
    container-type: size;
    padding: 0 8%;
  }
  .icon {
    font-size: 30cqh;
    line-height: 1;
  }
  .name {
    font-size: 9cqh;
    max-width: 80cqw;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .time {
    font-size: 8cqh;
    font-variant-numeric: tabular-nums;
    opacity: 0.8;
  }
  .bar {
    width: 70cqw;
    height: 3cqh;
    border-radius: 2cqh;
    background: rgba(255, 255, 255, 0.2);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: #4f8cff;
  }
</style>
