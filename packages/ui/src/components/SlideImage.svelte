<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!--
  Shows an image and swaps to a new `src` only after it has fully loaded and decoded.
  On error the previous frame stays visible: a broken image is never shown.
-->
<script lang="ts">
  interface Props {
    src: string | null;
    alt?: string;
    /** Background shown when there is no image (e.g. blank cues). */
    background?: string;
    fit?: "contain" | "cover";
    /** Called when a new image failed to load. */
    onerror?: (src: string) => void;
  }

  let { src, alt = "", background = "#000", fit = "contain", onerror }: Props = $props();

  let shown = $state<string | null>(null);
  let generation = 0;

  $effect(() => {
    const next = src;
    const gen = ++generation;
    if (!next) {
      shown = null;
      return;
    }
    if (next === shown) return;
    const img = new Image();
    img.decoding = "async";
    img.src = next;
    img
      .decode()
      .then(() => {
        if (gen === generation) shown = next;
      })
      .catch(() => {
        if (gen === generation) onerror?.(next);
      });
  });
</script>

<div class="frame" style:background>
  {#if shown}
    <img src={shown} {alt} style:object-fit={fit} draggable="false" />
  {/if}
</div>

<style>
  .frame {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    user-select: none;
  }
</style>
