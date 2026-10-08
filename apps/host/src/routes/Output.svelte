<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  interface Props {
    outputId: string;
  }
  let { outputId }: Props = $props();
</script>

<!-- Audience-facing output: no chrome, no cursor, black by default. -->
<div class="output" data-output={outputId}></div>

<style>
  :global(html),
  :global(body) {
    background: #000;
    cursor: none;
    overflow: hidden;
  }
  .output {
    position: fixed;
    inset: 0;
    background: #000;
  }
</style>
