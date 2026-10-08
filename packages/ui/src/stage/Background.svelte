<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Theme background (color + optional image). Calls `onready` once the image is decoded. -->
<script lang="ts">
  interface Props {
    color: string;
    src: string | null;
    onready: () => void;
  }
  let { color, src, onready }: Props = $props();
  let shown = $state<string | null>(null);

  $effect(() => {
    const next = src;
    if (!next) {
      shown = null;
      onready();
      return;
    }
    let cancelled = false;
    const img = new Image();
    img.src = next;
    img
      .decode()
      .then(() => {
        if (!cancelled) shown = next;
      })
      // Without the image the color background is still fine to show.
      .catch(() => {})
      .finally(() => {
        if (!cancelled) onready();
      });
    return () => (cancelled = true);
  });
</script>

<div class="bg" style:background={color}>
  {#if shown}<img src={shown} alt="" draggable="false" />{/if}
</div>

<style>
  .bg {
    position: absolute;
    inset: 0;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
</style>
