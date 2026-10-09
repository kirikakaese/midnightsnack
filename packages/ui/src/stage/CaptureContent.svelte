<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!--
  Live capture as an MJPEG stream. The browser keeps showing the last frame if the stream
  stalls (source lost), which is exactly what the output should do.
-->
<script lang="ts">
  interface Props {
    src: string;
    fit: "contain" | "cover" | "fill";
    onready: () => void;
  }
  let { src, fit, onready }: Props = $props();
  let img = $state<HTMLImageElement | null>(null);

  // Multipart images do not fire `load` reliably per frame everywhere: poll for the first frame.
  $effect(() => {
    const el = img;
    if (!el) return;
    const timer = setInterval(() => {
      if (el.naturalWidth > 0) {
        clearInterval(timer);
        onready();
      }
    }, 50);
    return () => clearInterval(timer);
  });
</script>

<img bind:this={img} {src} alt="" draggable="false" style:object-fit={fit} />

<style>
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    background: #000;
  }
</style>
