<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import Operator from "./routes/Operator.svelte";
  import Output from "./routes/Output.svelte";

  // Each Tauri window loads the same bundle; the hash selects the view.
  const route = window.location.hash.replace(/^#\/?/, "");
  const outputMatch = /^output\/(.+)$/.exec(route);
</script>

{#if outputMatch?.[1]}
  <Output outputId={outputMatch[1]} />
{:else}
  <Operator />
{/if}
