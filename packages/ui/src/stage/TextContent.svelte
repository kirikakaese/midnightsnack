<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { TextTheme } from "@midnightsnack/protocol";
  import Background from "./Background.svelte";
  import { fitText } from "./fit";

  interface Props {
    text: string;
    theme: TextTheme;
    backgroundSrc: string | null;
    onready: () => void;
  }
  let { text, theme, backgroundSrc, onready }: Props = $props();
</script>

<div class="slide" style:color={theme.color} style:font-family={theme.font_family}>
  <Background color={theme.background} src={backgroundSrc} {onready} />
  <div class="safe">
    <div class="text" style:text-align={theme.align} use:fitText={{ text, size: theme.font_size }}>
      {text}
    </div>
  </div>
</div>

<style>
  .slide {
    position: absolute;
    inset: 0;
    overflow: hidden;
  }
  .safe {
    position: absolute;
    inset: 6% 7%;
    display: flex;
    align-items: center;
  }
  .text {
    width: 100%;
    max-height: 100%;
    white-space: pre-line;
    overflow-wrap: break-word;
    line-height: 1.25;
    font-weight: 600;
    text-shadow: 0 0.04em 0.12em rgba(0, 0, 0, 0.45);
  }
</style>
