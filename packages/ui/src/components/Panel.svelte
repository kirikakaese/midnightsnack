<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    title?: string;
    children: Snippet;
    actions?: Snippet;
  }

  let { title, children, actions }: Props = $props();
</script>

<section class="ms-panel" aria-label={title}>
  {#if title || actions}
    <header>
      {#if title}<h2>{title}</h2>{/if}
      {#if actions}<div class="actions">{@render actions()}</div>{/if}
    </header>
  {/if}
  <div class="body">{@render children()}</div>
</section>

<style>
  .ms-panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--ms-surface);
    border: 1px solid var(--ms-border);
    border-radius: var(--ms-radius);
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 6px var(--ms-gap);
    padding: 8px 12px;
    border-bottom: 1px solid var(--ms-border);
  }
  h2 {
    margin: 0;
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ms-text-muted);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 12px;
  }
</style>
