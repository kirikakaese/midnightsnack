<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!--
  Renders what the audience sees: content (with transitions), overlays, logo, blackout.
  Used full size by output windows, as monitors in the operator view and on phones.

  A new frame is only revealed once it is ready (image decoded, video frame available); until
  then — or forever, if it fails — the previous frame stays up.
-->
<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import type { HostConnection } from "../client/connection.svelte";
  import { Ticker } from "../client/time.svelte";
  import { describe, effectiveTransition, type Content } from "../stage/content";
  import Layer from "../stage/Layer.svelte";
  import Overlays from "../stage/Overlays.svelte";
  import Logo from "./Logo.svelte";

  interface Props {
    conn: HostConnection;
    /** Pixel size to request slides at; omit for thumbnails. */
    width?: number;
    height?: number;
    /** Show master states and overlays like the audience sees them. */
    masters?: boolean;
    /** Which position to show. */
    which?: "output" | "program" | "next";
    /** `output` plays media with sound; `monitor` mirrors it muted; `thumb` never decodes media. */
    mode?: "output" | "monitor" | "thumb";
  }

  let { conn, width, height, masters = true, which = "output", mode = "monitor" }: Props = $props();

  interface LayerState {
    id: number;
    content: Content;
    ready: boolean;
    duration: number;
  }

  const BLACK: Content = { kind: "blank", key: "none", background: "#000" };
  const ticker = new Ticker(250);
  onDestroy(() => ticker.stop());
  const hostNow = $derived(ticker.now + conn.clockOffset);

  const pos = $derived(conn.live?.[which] ?? null);
  const content = $derived(describe(conn, pos, width, height) ?? BLACK);
  const live = $derived(which !== "next");

  let layers = $state<LayerState[]>([]);
  let nextId = 0;
  let cleanup: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    const c = content;
    untrack(() => {
      const top = layers[layers.length - 1];
      if (top && top.content.key === c.key) {
        // Same slide, updated data (e.g. edited text): replace in place.
        top.content = c;
        return;
      }
      layers.push({ id: nextId++, content: c, ready: false, duration: 0 });
    });
  });

  function onReady(layer: LayerState) {
    if (layer.ready) return;
    const index = layers.findIndex((l) => l.id === layer.id);
    if (index < 0) return;
    const cueId = layer.content.key.split(":")[0];
    const t = effectiveTransition(conn.show, conn.cue(cueId));
    // Thumbnails and the very first frame always cut.
    const fade = t.kind === "fade" && t.duration_ms > 0 && mode !== "thumb" && index > 0;
    layer.duration = fade ? t.duration_ms : 0;
    layer.ready = true;
    clearTimeout(cleanup);
    const id = layer.id;
    // Drop everything underneath once the new frame is fully visible.
    cleanup = setTimeout(() => {
      const i = layers.findIndex((l) => l.id === id);
      if (i > 0) layers.splice(0, i);
    }, layer.duration + 50);
  }

  // Output windows fetch the start of the next video/audio cue in advance.
  const preload = $derived.by(() => {
    if (mode !== "output") return null;
    const next = describe(conn, conn.live?.next ?? null);
    return next?.kind === "media" && next.key !== content.key ? next.src : null;
  });

  const masterFade = $derived(
    conn.show?.default_transition.kind === "fade" ? conn.show.default_transition.duration_ms : 0,
  );
  const blackout = $derived(masters && !!conn.live?.masters.blackout);
  const logo = $derived(masters && !!conn.live?.masters.logo);
  const logoSrc = $derived(conn.assetUrl(conn.show?.logo));
</script>

<div class="stage" style:--master-fade="{mode === 'thumb' ? 0 : masterFade}ms">
  {#each layers as layer (layer.id)}
    <div class="layer" class:shown={layer.ready} style:--fade="{layer.duration}ms">
      <Layer
        {conn}
        content={layer.content}
        {mode}
        {live}
        {hostNow}
        onready={() => onReady(layer)}
      />
    </div>
  {/each}
  {#if masters}
    <Overlays {conn} {hostNow} />
  {/if}
  <div class="master logo" class:on={logo}>
    {#if logoSrc}
      <img src={logoSrc} alt="" />
    {:else}
      <Logo size="30%" />
    {/if}
  </div>
  <div class="master blackout" class:on={blackout}></div>
  {#if preload}
    <video class="preload" src={preload} preload="auto" muted aria-hidden="true"></video>
  {/if}
</div>

<style>
  .stage {
    position: relative;
    width: 100%;
    height: 100%;
    background: #000;
    overflow: hidden;
    container-type: size;
  }
  .layer {
    position: absolute;
    inset: 0;
    opacity: 0;
    transition: opacity var(--fade) linear;
    will-change: opacity;
  }
  .layer.shown {
    opacity: 1;
  }
  .master {
    position: absolute;
    inset: 0;
    opacity: 0;
    visibility: hidden;
    transition:
      opacity var(--master-fade) linear,
      visibility 0s linear var(--master-fade);
  }
  .master.on {
    opacity: 1;
    visibility: visible;
    transition:
      opacity var(--master-fade) linear,
      visibility 0s;
  }
  .logo {
    display: grid;
    place-items: center;
    background: #000;
  }
  .logo img {
    max-width: 60%;
    max-height: 60%;
    object-fit: contain;
  }
  .blackout {
    background: #000;
  }
  .preload {
    position: absolute;
    width: 1px;
    height: 1px;
    opacity: 0;
    pointer-events: none;
  }
</style>
