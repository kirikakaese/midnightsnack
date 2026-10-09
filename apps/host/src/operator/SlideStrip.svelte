<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Thumbnails of every slide in the live cue; click to jump. -->
<script lang="ts">
  import { Panel, SlidePreview, t, type HostConnection } from "@midnightsnack/ui";

  interface Props {
    conn: HostConnection;
  }
  let { conn }: Props = $props();

  const cueId = $derived(conn.live?.program?.cue_id ?? conn.live?.next?.cue_id ?? null);
  const cue = $derived(conn.cue(cueId));
  const current = $derived(conn.live?.program?.cue_id === cueId ? conn.live?.program?.slide : -1);
  let strip = $state<HTMLElement | null>(null);

  $effect(() => {
    const el = strip?.querySelector<HTMLElement>(`[data-slide="${current}"]`);
    el?.scrollIntoView({ block: "nearest", inline: "center", behavior: "smooth" });
  });
</script>

<Panel title={cue ? t("slides.of_cue", { name: cue.name }) : t("slides.title")}>
  {#if cue}
    <div class="strip" bind:this={strip}>
      {#each Array.from({ length: cue.slide_count }, (_, i) => i) as slide (slide)}
        <button
          class="thumb"
          class:current={slide === current}
          data-slide={slide}
          aria-label={t("slides.go_to", { n: slide + 1 })}
          aria-current={slide === current ? "true" : undefined}
          onclick={() => conn.action({ action: "go_to", position: { cue_id: cue.id, slide } })}
        >
          <SlidePreview {conn} position={{ cue_id: cue.id, slide }} />
          <span class="n">{slide + 1}</span>
        </button>
      {/each}
    </div>
  {/if}
</Panel>

<style>
  .strip {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    padding-bottom: 4px;
  }
  .thumb {
    position: relative;
    flex: 0 0 160px;
    aspect-ratio: 16 / 9;
    padding: 0;
    border: 3px solid var(--ms-border);
    border-radius: var(--ms-radius-sm);
    overflow: hidden;
    cursor: pointer;
    background: #000;
  }
  .thumb.current {
    border-color: var(--ms-go);
  }
  .n {
    position: absolute;
    left: 4px;
    bottom: 2px;
    padding: 0 4px;
    border-radius: 3px;
    background: rgba(0, 0, 0, 0.7);
    color: #fff;
    font-size: 0.75rem;
  }
</style>
