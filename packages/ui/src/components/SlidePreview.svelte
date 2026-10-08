<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Static thumbnail of one slide of any cue type (no media decoding, no live timeline). -->
<script lang="ts">
  import type { Position } from "@midnightsnack/protocol";
  import { onDestroy } from "svelte";
  import type { HostConnection } from "../client/connection.svelte";
  import { Ticker } from "../client/time.svelte";
  import { describe } from "../stage/content";
  import Layer from "../stage/Layer.svelte";

  interface Props {
    conn: HostConnection;
    position: Position;
  }
  let { conn, position }: Props = $props();
  const content = $derived(describe(conn, position));
  // Timer and clock previews still tick, slowly.
  const ticker = new Ticker(1000);
  onDestroy(() => ticker.stop());
</script>

<div class="preview">
  {#if content}
    <Layer
      {conn}
      {content}
      mode="thumb"
      live={false}
      hostNow={ticker.now + conn.clockOffset}
      onready={() => {}}
    />
  {/if}
</div>

<style>
  .preview {
    position: absolute;
    inset: 0;
    background: #000;
    container-type: size;
  }
</style>
