<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  interface Props {
    src: string;
    background: string;
    fit?: "contain" | "cover" | "fill";
    onready: () => void;
  }
  let { src, background, fit = "contain", onready }: Props = $props();
  let shown = $state<string | null>(null);

  $effect(() => {
    const next = src;
    let cancelled = false;
    const img = new Image();
    img.decoding = "async";
    img.src = next;
    img
      .decode()
      .then(() => {
        if (cancelled) return;
        shown = next;
        onready();
      })
      // A broken image never becomes ready, so the previous frame stays up.
      .catch(() => {});
    return () => (cancelled = true);
  });
</script>

<div class="fill" style:background>
  {#if shown}<img src={shown} alt="" draggable="false" style:object-fit={fit} />{/if}
</div>

<style>
  .fill {
    position: absolute;
    inset: 0;
  }
  img {
    width: 100%;
    height: 100%;
    user-select: none;
  }
</style>
