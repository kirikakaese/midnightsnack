<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Setup patterns: a geometry grid with the real output resolution, or color bars. -->
<script lang="ts">
  import type { TestPattern } from "@midnightsnack/protocol";

  interface Props {
    pattern: TestPattern;
    label: string;
    width?: number;
    height?: number;
  }
  let { pattern, label, width, height }: Props = $props();
  const BARS = ["#c0c0c0", "#c0c000", "#00c0c0", "#00c000", "#c000c0", "#c00000", "#0000c0"];
</script>

<div class="pattern">
  {#if pattern === "bars"}
    <div class="bars">
      {#each BARS as c (c)}<div style:background={c}></div>{/each}
    </div>
    <div class="ramp"></div>
  {:else}
    <svg class="grid" viewBox="0 0 160 90" preserveAspectRatio="none" aria-hidden="true">
      {#each Array.from({ length: 17 }, (_, i) => i * 10) as x (x)}
        <line x1={x} y1="0" x2={x} y2="90" />
      {/each}
      {#each Array.from({ length: 10 }, (_, i) => i * 10) as y (y)}
        <line x1="0" y1={y} x2="160" y2={y} />
      {/each}
      <rect x="0.4" y="0.4" width="159.2" height="89.2" class="edge" />
      <line x1="0" y1="0" x2="160" y2="90" class="diag" />
      <line x1="160" y1="0" x2="0" y2="90" class="diag" />
    </svg>
    <div class="circle"></div>
  {/if}
  <div class="info">
    <strong>{label}</strong>
    {#if width && height}<span>{width} × {height}</span>{/if}
  </div>
</div>

<style>
  .pattern {
    position: absolute;
    inset: 0;
    background: #000;
    container-type: size;
    overflow: hidden;
  }
  .grid {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  .grid line {
    stroke: #fff;
    stroke-width: 0.15;
    vector-effect: non-scaling-stroke;
  }
  .grid .diag {
    stroke: #666;
  }
  .grid .edge {
    fill: none;
    stroke: #ffd447;
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
  }
  .circle {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 80cqh;
    height: 80cqh;
    transform: translate(-50%, -50%);
    border: 0.4cqh solid #fff;
    border-radius: 50%;
  }
  .bars {
    position: absolute;
    inset: 0 0 25% 0;
    display: grid;
    grid-template-columns: repeat(7, 1fr);
  }
  .ramp {
    position: absolute;
    inset: 75% 0 0 0;
    background: linear-gradient(to right, #000, #fff);
  }
  .info {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    display: grid;
    justify-items: center;
    padding: 2cqh 4cqh;
    background: rgba(0, 0, 0, 0.75);
    color: #fff;
    font-family: system-ui, sans-serif;
    font-size: 5cqh;
    border-radius: 1cqh;
  }
  .info span {
    font-size: 8cqh;
    font-variant-numeric: tabular-nums;
    font-weight: 700;
  }
</style>
