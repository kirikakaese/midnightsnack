<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts" generics="T extends string">
  import type { Snippet } from "svelte";

  interface Props {
    tabs: { id: T; label: string; badge?: number }[];
    active: T;
    label: string;
    children: Snippet;
  }
  let { tabs, active = $bindable(), label, children }: Props = $props();

  function onKey(e: KeyboardEvent) {
    const i = tabs.findIndex((t) => t.id === active);
    const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    e.stopPropagation();
    const next = tabs[(i + step + tabs.length) % tabs.length];
    if (next) active = next.id;
  }
</script>

<div class="tabs">
  <div class="list" role="tablist" aria-label={label} tabindex="-1" onkeydown={onKey}>
    {#each tabs as tab (tab.id)}
      <button
        role="tab"
        aria-selected={tab.id === active}
        tabindex={tab.id === active ? 0 : -1}
        class:active={tab.id === active}
        onclick={() => (active = tab.id)}
      >
        {tab.label}
        {#if tab.badge}<span class="badge">{tab.badge}</span>{/if}
      </button>
    {/each}
  </div>
  <div class="panel" role="tabpanel">{@render children()}</div>
</div>

<style>
  .tabs {
    display: grid;
    grid-template-rows: auto 1fr;
    min-height: 0;
    background: var(--ms-surface);
    border: 1px solid var(--ms-border);
    border-radius: var(--ms-radius);
    overflow: hidden;
  }
  .list {
    display: flex;
    border-bottom: 1px solid var(--ms-border);
    overflow-x: auto;
  }
  button {
    flex: 1;
    min-height: 40px;
    padding: 0 10px;
    border: 0;
    border-bottom: 3px solid transparent;
    background: transparent;
    color: var(--ms-text-muted);
    font: inherit;
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    cursor: pointer;
    white-space: nowrap;
  }
  button.active {
    color: var(--ms-text);
    border-bottom-color: var(--ms-accent);
  }
  .badge {
    display: inline-block;
    min-width: 18px;
    margin-left: 4px;
    padding: 0 5px;
    border-radius: 9px;
    background: var(--ms-warn);
    color: #241500;
    font-size: 0.75rem;
  }
  .panel {
    min-height: 0;
    overflow: auto;
    padding: 12px;
  }
</style>
