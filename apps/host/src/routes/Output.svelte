<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!--
  Audience-facing output. Renders only what the host says is on air. If the connection drops,
  the last frame stays up; nothing else (errors, cursors, spinners) is ever drawn here.
-->
<script lang="ts">
  import { Stage, type HostConnection } from "@midnightsnack/ui";
  import { onDestroy } from "svelte";
  import { connectToHost, host } from "../lib/host";
  import { buildKeymap, installKeyHandler, type KeyAction } from "../lib/keymap";

  interface Props {
    outputId: string;
  }
  let { outputId }: Props = $props();

  let conn = $state<HostConnection | null>(null);
  let keymap = $state<Record<string, KeyAction>>(buildKeymap({}));
  let size = $state({ width: window.innerWidth, height: window.innerHeight });

  const pixelSize = $derived({
    width: Math.round(size.width * window.devicePixelRatio),
    height: Math.round(size.height * window.devicePixelRatio),
  });

  connectToHost().then((c) => (conn = c));
  host.keymap().then((k) => (keymap = buildKeymap(k)));

  const onResize = () => (size = { width: window.innerWidth, height: window.innerHeight });
  window.addEventListener("resize", onResize);
  // A clicker may send keys to the output window if it ever gets focus.
  const removeKeys = installKeyHandler(
    () => keymap,
    (a) => conn?.action(a),
  );
  // Never show a context menu on the audience screen.
  const noMenu = (e: Event) => e.preventDefault();
  window.addEventListener("contextmenu", noMenu);

  $effect(() => {
    conn?.reportViewport(pixelSize.width, pixelSize.height);
  });

  onDestroy(() => {
    window.removeEventListener("resize", onResize);
    window.removeEventListener("contextmenu", noMenu);
    removeKeys();
    conn?.close();
  });
</script>

<div class="output" data-output={outputId}>
  {#if conn}
    <Stage {conn} width={pixelSize.width} height={pixelSize.height} mode="output" />
  {/if}
</div>

<style>
  :global(html),
  :global(body) {
    background: #000;
    cursor: none;
    overflow: hidden;
    user-select: none;
  }
  :global(*) {
    cursor: none !important;
  }
  .output {
    position: fixed;
    inset: 0;
    background: #000;
  }
</style>
