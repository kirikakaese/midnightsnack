<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!--
  Drawings and laser pointers on top of the slide. Coordinates are fractions of a 16:9 frame
  fitted into the stage, the same frame the pointer pad uses on remotes.
-->
<script lang="ts" module>
  export const FRAME_W = 1600;
  export const FRAME_H = 900;

  export function pathOf(points: [number, number][]): string {
    return points
      .map(([x, y]) => `${(x * FRAME_W).toFixed(1)},${(y * FRAME_H).toFixed(1)}`)
      .join(" ");
  }
</script>

<script lang="ts">
  import type { Position, Stroke } from "@midnightsnack/protocol";
  import type { HostConnection, RemotePointer } from "../client/connection.svelte";

  interface Props {
    conn: HostConnection;
    /** The slide this stage shows; drawings for other slides are not drawn. */
    position: Position | null;
    /** Show other devices' pointers (not on previews of the next slide). */
    pointers?: boolean;
    /** This device's own pointer and stroke in progress (pointer pad). */
    local?: RemotePointer | null;
  }
  let { conn, position, pointers = true, local = null }: Props = $props();

  const strokes = $derived.by((): Stroke[] => {
    const d = conn.live?.drawing;
    if (!d || !position) return [];
    return d.position.cue_id === position.cue_id && d.position.slide === position.slide
      ? d.strokes
      : [];
  });
  const shown = $derived([
    ...(pointers && position ? Object.values(conn.pointers) : []),
    ...(local ? [local] : []),
  ]);
</script>

<svg
  class="pointer-layer"
  viewBox="0 0 {FRAME_W} {FRAME_H}"
  preserveAspectRatio="xMidYMid meet"
  aria-hidden="true"
>
  {#each strokes as s, i (i)}
    <polyline
      points={pathOf(s.points)}
      stroke={s.color}
      stroke-width={s.width * FRAME_H}
      fill="none"
      stroke-linecap="round"
      stroke-linejoin="round"
    />
  {/each}
  {#each shown as p, i (i)}
    {#if p.mode === "draw" && p.trail.length > 1}
      <polyline
        points={pathOf(p.trail)}
        stroke={p.color}
        stroke-width={0.006 * FRAME_H}
        fill="none"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    {:else if p.mode === "point"}
      <circle
        class="laser"
        cx={p.pos[0] * FRAME_W}
        cy={p.pos[1] * FRAME_H}
        r={0.012 * FRAME_H}
        fill={p.color}
        style:--glow={p.color}
      />
    {/if}
  {/each}
</svg>

<style>
  .pointer-layer {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    overflow: hidden;
  }
  .laser {
    filter: drop-shadow(0 0 6px var(--glow)) drop-shadow(0 0 2px #fff);
    opacity: 0.9;
  }
</style>
