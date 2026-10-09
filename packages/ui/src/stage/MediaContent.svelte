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
    fit?: "contain" | "cover" | "fill";
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
    fit = "contain",
    onready,
  }: Props = $props();

  /** Drift that is corrected by seeking (ms). Smaller drift is absorbed by the playback rate. */
  const SEEK_DRIFT_MS = 1000;
  /** Drift below this is ignored (ms). */
  const TOLERANCE_MS = 60;
  /** Never seek more often than this (ms): seeking is expensive and restarts decoding. */
  const SEEK_COOLDOWN_MS = 2000;
  let lastSeek = 0;

  let el = $state<HTMLMediaElement | null>(null);
  /** Bumped when the element can be synced (metadata loaded), to re-run the sync effect. */
  let loaded = $state(0);
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

  // Keep the element on the host's timeline: small drift is corrected by playing slightly
  // faster or slower (inaudible), large drift (seek, restart, pause) by seeking.
  $effect(() => {
    // Read every reactive input before any early return: Svelte only re-runs an effect for
    // the state it actually read.
    const m = el;
    const target = desiredMs;
    const shouldPlay = playing;
    const volume = mode === "output" ? Math.max(0, Math.min(1, options.volume)) : 0;
    void loaded;
    if (!m || m.readyState < 1) return;
    m.volume = volume;
    const drift = target - m.currentTime * 1000;
    const now = performance.now();
    if (!shouldPlay) {
      if (!m.paused) m.pause();
      if (Math.abs(drift) > TOLERANCE_MS) m.currentTime = target / 1000;
      return;
    }
    if (Math.abs(drift) > SEEK_DRIFT_MS && now - lastSeek > SEEK_COOLDOWN_MS) {
      lastSeek = now;
      m.currentTime = target / 1000;
      m.playbackRate = 1;
    } else if (Math.abs(drift) > TOLERANCE_MS) {
      m.playbackRate = 1 + Math.max(-0.08, Math.min(0.08, drift / 2000));
    } else {
      m.playbackRate = 1;
    }
    if (m.paused) m.play().catch(() => {});
  });

  function onLoaded() {
    const m = el;
    if (!m) return;
    lastSeek = performance.now();
    m.currentTime = desiredMs / 1000;
    loaded++;
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
    style:object-fit={fit}
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
