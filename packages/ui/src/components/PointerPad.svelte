<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!--
  Turns a stage view into a laser pointer / drawing surface. Positions are fractions of the
  16:9 frame the outputs draw pointers in. With `mode="off"` it is transparent to input.
-->
<script lang="ts">
  import type { PointerMode } from "@midnightsnack/protocol";
  import { untrack, type Snippet } from "svelte";
  import type { HostConnection, RemotePointer } from "../client/connection.svelte";
  import PointerLayer from "../stage/PointerLayer.svelte";

  interface Props {
    conn: HostConnection;
    mode: "off" | PointerMode;
    color: string;
    /** Stroke width as a fraction of the slide height. */
    width?: number;
    children: Snippet;
  }
  let { conn, mode, color, width = 0.006, children }: Props = $props();

  const MAX_POINTS = 2000;
  /** Minimum distance between recorded points (fraction of the frame). */
  const MIN_STEP = 0.002;

  let pad = $state<HTMLDivElement>();
  let local = $state<RemotePointer | null>(null);
  let down = false;

  /** Event position as a fraction of the 16:9 frame fitted into the pad. */
  function frame(e: PointerEvent): [number, number] {
    const r = pad!.getBoundingClientRect();
    const fw = Math.min(r.width, (r.height * 16) / 9);
    const fh = (fw * 9) / 16;
    const x = (e.clientX - r.left - (r.width - fw) / 2) / fw;
    const y = (e.clientY - r.top - (r.height - fh) / 2) / fh;
    return [Math.min(1, Math.max(0, x)), Math.min(1, Math.max(0, y))];
  }

  function start(e: PointerEvent) {
    if (mode === "off" || !e.isPrimary) return;
    e.preventDefault();
    pad!.setPointerCapture(e.pointerId);
    down = true;
    const p = frame(e);
    local = { pos: p, mode, color, trail: [p], at: performance.now() };
    conn.sendPointer(p, mode, color);
  }

  function move(e: PointerEvent) {
    if (!down || !local || !e.isPrimary) return;
    const p = frame(e);
    const last = local.trail[local.trail.length - 1]!;
    if (Math.hypot(p[0] - last[0], p[1] - last[1]) < MIN_STEP) return;
    const trail = local.mode === "draw" ? [...local.trail, p] : [p];
    local = { ...local, pos: p, trail };
    conn.sendPointer(p, local.mode, color);
  }

  function end(e: PointerEvent) {
    if (!down || !e.isPrimary) return;
    down = false;
    const finished = local;
    if (finished?.mode === "draw") {
      let points = finished.trail;
      // Thin out very long strokes so they fit the protocol limit.
      if (points.length > MAX_POINTS) {
        const step = Math.ceil(points.length / MAX_POINTS);
        points = points.filter((_, i) => i % step === 0);
      }
      void conn.action({ action: "draw_stroke", stroke: { color, width, points } });
      // Fallback if the host refused the stroke.
      setTimeout(() => {
        if (!down && local === finished) local = null;
      }, 1500);
    } else {
      local = null;
    }
    conn.sendPointer(null, finished?.mode ?? "point", color);
  }

  // Keep a finished stroke on screen until the host's drawing (which includes it) arrives.
  $effect(() => {
    void conn.live?.drawing;
    untrack(() => {
      if (!down && local?.mode === "draw") local = null;
    });
  });
</script>

<div
  class="pad"
  class:active={mode !== "off"}
  bind:this={pad}
  onpointerdown={start}
  onpointermove={move}
  onpointerup={end}
  onpointercancel={end}
  role="presentation"
>
  {@render children()}
  <PointerLayer {conn} position={null} pointers={false} {local} />
</div>

<style>
  .pad {
    position: relative;
    width: 100%;
    height: 100%;
  }
  .pad.active {
    touch-action: none;
    cursor: crosshair;
    user-select: none;
  }
</style>
