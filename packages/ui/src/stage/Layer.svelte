<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { HostConnection } from "../client/connection.svelte";
  import type { Content } from "./content";
  import ImageContent from "./ImageContent.svelte";
  import MediaContent from "./MediaContent.svelte";
  import TextContent from "./TextContent.svelte";
  import TimerContent from "./TimerContent.svelte";

  interface Props {
    conn: HostConnection;
    content: Content;
    mode: "output" | "monitor" | "thumb";
    /** This layer shows the live output (media follows the host timeline). */
    live: boolean;
    hostNow: number;
    onready: () => void;
  }
  let { conn, content, mode, live, hostNow, onready }: Props = $props();

  const playback = $derived(
    live && content.kind === "media" && conn.live?.media?.cue_id === content.cueId
      ? conn.live.media
      : null,
  );
  const timerElapsed = $derived(
    content.kind === "timer" && conn.live?.program?.cue_id === content.cueId
      ? conn.live.slide_timer
      : null,
  );

  $effect(() => {
    if (content.kind === "blank") onready();
  });
</script>

{#if content.kind === "image"}
  <ImageContent src={content.src} background={content.background} {onready} />
{:else if content.kind === "blank"}
  <div class="blank" style:background={content.background}></div>
{:else if content.kind === "media"}
  <MediaContent
    {conn}
    cueId={content.cueId}
    video={content.video}
    src={content.src}
    name={content.name}
    options={content.options}
    durationMs={content.durationMs}
    {playback}
    {mode}
    {hostNow}
    {onready}
  />
{:else if content.kind === "text"}
  <TextContent
    text={content.text}
    theme={content.theme}
    backgroundSrc={content.backgroundSrc}
    {onready}
  />
{:else if content.kind === "timer"}
  <TimerContent
    timer={content.timer}
    theme={content.theme}
    backgroundSrc={content.backgroundSrc}
    elapsed={timerElapsed}
    {hostNow}
    {onready}
  />
{/if}

<style>
  .blank {
    position: absolute;
    inset: 0;
  }
</style>
