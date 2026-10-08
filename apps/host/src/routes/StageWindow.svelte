<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Stage display window on the host (for a confidence monitor facing the presenter). -->
<script lang="ts">
  import { StageDisplay, type HostConnection } from "@midnightsnack/ui";
  import { onDestroy } from "svelte";
  import { connectToHost } from "../lib/host";

  let conn = $state<HostConnection | null>(null);
  connectToHost().then((c) => (conn = c));
  onDestroy(() => conn?.close());
</script>

<main class="stage-window">
  {#if conn}<StageDisplay {conn} mode="monitor" />{/if}
</main>

<style>
  .stage-window {
    min-height: 100vh;
    padding: 16px;
    background: var(--ms-bg);
  }
</style>
