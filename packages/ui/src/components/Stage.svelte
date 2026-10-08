<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!--
  Renders what the audience sees for a live state: content, then logo, then blackout on top.
  Used by output windows (full size) and as the "program" monitor in the operator view.
-->
<script lang="ts">
  import type { HostConnection } from "../client/connection.svelte";
  import Logo from "./Logo.svelte";
  import SlideImage from "./SlideImage.svelte";

  interface Props {
    conn: HostConnection;
    /** Pixel size to request; omit for thumbnails. */
    width?: number;
    height?: number;
    /** Show master states (blackout/logo) like the audience sees them. */
    masters?: boolean;
    /** Which position to show; defaults to the output position. */
    which?: "output" | "program" | "next";
  }

  let { conn, width, height, masters = true, which = "output" }: Props = $props();

  const pos = $derived(conn.live?.[which] ?? null);
  const cue = $derived(conn.cue(pos?.cue_id));
  const src = $derived(conn.slideUrl(pos, width, height));
  const background = $derived(cue?.background ?? "#000");
  const blackout = $derived(masters && !!conn.live?.masters.blackout);
  const logo = $derived(masters && !!conn.live?.masters.logo);
</script>

<div class="stage">
  <SlideImage {src} {background} />
  {#if logo}
    <div class="layer logo"><Logo size="30%" /></div>
  {/if}
  {#if blackout}
    <div class="layer blackout"></div>
  {/if}
</div>

<style>
  .stage {
    position: relative;
    width: 100%;
    height: 100%;
    background: #000;
    overflow: hidden;
  }
  .layer {
    position: absolute;
    inset: 0;
  }
  .logo {
    display: grid;
    place-items: center;
    background: #000;
  }
  .blackout {
    background: #000;
  }
</style>
