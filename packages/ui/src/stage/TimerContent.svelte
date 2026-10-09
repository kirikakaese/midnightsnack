<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { Stopwatch, TextTheme, TimerCue } from "@midnightsnack/protocol";
  import { elapsedMs, formatClock, formatDuration } from "../client/time.svelte";
  import Background from "./Background.svelte";
  import { untilClockTime } from "./content";

  interface Props {
    timer: TimerCue;
    theme: TextTheme;
    backgroundSrc: string | null;
    /** Time on this cue (the slide timer while it is live). */
    elapsed: Stopwatch | null;
    hostNow: number;
    onready: () => void;
  }
  let { timer, theme, backgroundSrc, elapsed, hostNow, onready }: Props = $props();

  const value = $derived.by(() => {
    const e = elapsedMs(elapsed, hostNow);
    switch (timer.mode.mode) {
      case "countdown": {
        const left = timer.mode.duration_ms - e;
        return {
          text: (left < 0 ? "+" : "") + formatDuration(Math.abs(left) + (left > 0 ? 999 : 0)),
          over: left < 0,
        };
      }
      case "countdown_to": {
        const left = untilClockTime(timer.mode.time, new Date(hostNow));
        return {
          text: (left < 0 ? "+" : "") + formatDuration(Math.abs(left) + (left > 0 ? 999 : 0)),
          over: left < 0,
        };
      }
      case "count_up":
        return { text: formatDuration(e), over: false };
      case "clock":
        return { text: formatClock(hostNow), over: false };
    }
  });
</script>

<div class="slide" style:font-family={theme.font_family}>
  <Background color={theme.background} src={backgroundSrc} {onready} />
  <div class="center" style:color={value.over ? timer.overtime_color : theme.color}>
    {#if timer.label}<div class="label">{timer.label}</div>{/if}
    <div class="value">{value.text}</div>
  </div>
</div>

<style>
  .slide {
    position: absolute;
    inset: 0;
    overflow: hidden;
    container-type: size;
  }
  .center {
    position: absolute;
    inset: 0;
    display: grid;
    place-content: center;
    text-align: center;
  }
  .label {
    font-size: 8cqh;
    font-weight: 600;
    opacity: 0.85;
  }
  .value {
    font-size: 30cqh;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    line-height: 1.05;
  }
</style>
