<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { HostConnection } from "../client/connection.svelte";
  import type { Content } from "./content";
  import ImageContent from "./ImageContent.svelte";
  import MediaContent from "./MediaContent.svelte";
  import TextContent from "./TextContent.svelte";
  import TimerContent from "./TimerContent.svelte";
  import CaptureContent from "./CaptureContent.svelte";
  import WebPlaceholder from "./WebPlaceholder.svelte";

  interface Props {
    conn: HostConnection;
    content: Content;
    mode: "output" | "monitor" | "thumb";
    /** This layer shows the live output (media follows the host timeline). */
    live: boolean;
    hostNow: number;
    fit?: "contain" | "cover" | "fill";
    onready: () => void;
  }
  let { conn, content, mode, live, hostNow, fit = "contain", onready }: Props = $props();

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
    // On the output a web page is a separate webview on top; underneath is black.
    if (content.kind === "blank" || (content.kind === "web" && mode === "output")) onready();
  });
</script>

{#if content.kind === "image"}
  <ImageContent src={content.src} background={content.background} {fit} {onready} />
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
    {fit}
    {onready}
  />
{:else if content.kind === "web"}
  {#if mode === "output"}
    <div class="blank" style:background="#000"></div>
  {:else}
    <WebPlaceholder
      name={content.name}
      url={content.url}
      openslides={content.openslides}
      {onready}
    />
  {/if}
{:else if content.kind === "capture"}
  <CaptureContent src={content.src} {fit} {onready} />
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
