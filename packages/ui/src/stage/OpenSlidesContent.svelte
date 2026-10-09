<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- An OpenSlides slide drawn natively in the show's theme: agenda, motion, topic, list of
     speakers, or whatever a followed OpenSlides projector shows. -->
<script lang="ts">
  import type {
    OpenSlidesSlide,
    OsMeetingData,
    OsSpeaker,
    TextTheme,
  } from "@midnightsnack/protocol";
  import { formatDuration } from "../client/time.svelte";
  import { t, type MessageKey } from "../i18n/index.svelte";
  import Background from "./Background.svelte";
  import { fitText } from "./fit";
  import { resolveOpenSlides } from "./openslides";

  interface Props {
    data: OsMeetingData | null;
    slide: OpenSlidesSlide;
    page: number;
    theme: TextTheme;
    backgroundSrc: string | null;
    hostNow: number;
    onready: () => void;
  }
  let { data, slide, page, theme, backgroundSrc, hostNow, onready }: Props = $props();

  const view = $derived(resolveOpenSlides(data, slide, page));
  // Re-fit whenever the content (not the clock) changes.
  const signature = $derived(JSON.stringify(view));

  const SPEECH: Record<string, MessageKey> = {
    pro: "os.speech.pro",
    contra: "os.speech.contra",
    contribution: "os.speech.contribution",
    intervention: "os.speech.intervention",
    interposed_question: "os.speech.interposed_question",
  };
  const COLLECTION: Record<string, MessageKey> = {
    assignment: "os.collection.assignment",
    poll: "os.collection.poll",
    meeting_mediafile: "os.collection.mediafile",
    projector_message: "os.collection.message",
    projector_countdown: "os.collection.countdown",
    motion_block: "os.collection.motion_block",
  };

  function speech(s: OsSpeaker): string {
    if (s.point_of_order) return t("os.point_of_order");
    const key = s.speech_state ? SPEECH[s.speech_state] : undefined;
    return key ? t(key) : "";
  }

  function collectionName(c: string): string {
    const key = COLLECTION[c];
    return key ? t(key) : c;
  }

  /** Speaking time; hidden when the clocks of OpenSlides and the host disagree a lot. */
  function elapsed(s: OsSpeaker): string {
    if (s.begin_ms === null) return "";
    const ms = hostNow - s.begin_ms;
    return ms >= -5000 && ms < 12 * 3600 * 1000 ? formatDuration(Math.max(0, ms)) : "";
  }
</script>

<div class="slide" style:color={theme.color} style:font-family={theme.font_family}>
  <Background color={theme.background} src={backgroundSrc} {onready} />
  {#if view.kind === "empty" || view.kind === "other"}
    <div class="placeholder">
      <span class="logo" aria-hidden="true">◫</span>
      {#if view.kind === "other"}
        <strong>{view.title || collectionName(view.collection)}</strong>
        <span>{t("os.not_native")}</span>
      {:else}
        <span>{t(`os.empty.${view.reason}`)}</span>
      {/if}
    </div>
  {:else}
    <div class="safe">
      <div
        class="fit"
        style:--heading-align={theme.align}
        use:fitText={{ text: signature, size: theme.font_size, max: 6 }}
      >
        {#if view.kind === "agenda"}
          <h1>{t("os.agenda")}</h1>
          <ol class="agenda">
            {#each view.items as item (item.id)}
              <li class:closed={item.closed} style:padding-left="{item.level * 1.5}em">
                {#if item.number}<span class="num">{item.number}</span>{/if}
                <span class="title">{item.title}</span>
              </li>
            {/each}
          </ol>
        {:else if view.kind === "motion"}
          {#if view.first}
            <h1>
              {#if view.motion.number}<span class="num">{view.motion.number}</span>{/if}
              {view.motion.title}
            </h1>
            <p class="meta">
              {#if view.motion.submitters.length}
                {t("os.submitters", { names: view.motion.submitters.join(", ") })}
              {/if}
              {#if view.motion.state}
                <span class="state">{view.motion.state}</span>
              {/if}
            </p>
          {:else}
            <h2>{view.motion.number} {view.motion.title}</h2>
          {/if}
          {#each view.blocks as b, i (i)}
            {#if b.kind === "heading"}
              <h3>{b.text}</h3>
            {:else if b.kind === "reason_heading"}
              <h3>{t("os.reason")}</h3>
            {:else if b.kind === "list_item"}
              <p class="li">{b.text}</p>
            {:else}
              <p>{b.text}</p>
            {/if}
          {/each}
        {:else if view.kind === "topic"}
          {#if view.first}<h1>{view.topic.title}</h1>{:else}<h2>{view.topic.title}</h2>{/if}
          {#each view.blocks as b, i (i)}
            {#if b.kind === "heading"}
              <h3>{b.text}</h3>
            {:else if b.kind === "list_item"}
              <p class="li">{b.text}</p>
            {:else}
              <p>{b.text}</p>
            {/if}
          {/each}
        {:else if view.kind === "speakers"}
          <h1>{view.list.title || t("os.speakers")}</h1>
          {#if view.list.closed}<p class="meta">{t("os.list_closed")}</p>{/if}
          {#each view.last as s, i (i)}
            <p class="last">{s.name}</p>
          {/each}
          {#each view.current as s, i (i)}
            <p class="current">
              <span class="name">{s.name}</span>
              {#if speech(s)}<span class="tag">{speech(s)}</span>{/if}
              <span class="time">{elapsed(s)}</span>
            </p>
          {/each}
          {#if view.next.length}
            <ol class="next">
              {#each view.next as s, i (i)}
                <li>
                  {s.name}
                  {#if speech(s)}<span class="tag">{speech(s)}</span>{/if}
                </li>
              {/each}
            </ol>
            {#if view.more}<p class="meta">{t("os.more_speakers", { n: view.more })}</p>{/if}
          {:else if !view.current.length}
            <p class="meta">{t("os.no_speakers")}</p>
          {/if}
        {/if}
      </div>
    </div>
    {#if (view.kind === "agenda" || view.kind === "motion" || view.kind === "topic") && view.pages > 1}
      <span class="page">{t("os.page", { n: view.page + 1, total: view.pages })}</span>
    {/if}
  {/if}
</div>

<style>
  .slide {
    position: absolute;
    inset: 0;
    overflow: hidden;
    container-type: size;
  }
  .safe {
    position: absolute;
    inset: 6% 7%;
    display: flex;
    align-items: flex-start;
  }
  .fit {
    text-align: left;
    width: 100%;
    max-height: 100%;
    line-height: 1.3;
    overflow-wrap: break-word;
    text-shadow: 0 0.04em 0.12em rgba(0, 0, 0, 0.35);
  }
  h1 {
    text-align: var(--heading-align, left);
    margin: 0 0 0.4em;
    font-size: 1.7em;
    line-height: 1.15;
  }
  h2 {
    margin: 0 0 0.4em;
    font-size: 1.1em;
    opacity: 0.7;
  }
  h3 {
    margin: 0.6em 0 0.2em;
    font-size: 1.15em;
  }
  p {
    margin: 0 0 0.45em;
  }
  .li {
    padding-left: 1.2em;
    text-indent: -0.8em;
  }
  .li::before {
    content: "• ";
  }
  .num {
    font-weight: 700;
    margin-right: 0.5em;
  }
  .meta {
    opacity: 0.75;
    font-size: 0.85em;
  }
  .state {
    margin-left: 0.6em;
    padding: 0.05em 0.5em;
    border: 0.06em solid currentColor;
    border-radius: 0.4em;
  }
  .agenda {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .agenda li {
    margin: 0 0 0.35em;
  }
  .agenda .closed {
    opacity: 0.55;
  }
  .agenda .closed .title {
    text-decoration: line-through;
  }
  .last {
    opacity: 0.55;
    font-size: 0.85em;
  }
  .current {
    display: flex;
    align-items: baseline;
    gap: 0.6em;
    font-size: 1.5em;
    font-weight: 700;
    margin: 0.2em 0 0.5em;
  }
  .current .time {
    margin-left: auto;
    font-variant-numeric: tabular-nums;
  }
  .next {
    margin: 0;
    padding-left: 1.4em;
  }
  .next li {
    margin: 0 0 0.3em;
  }
  .tag {
    font-size: 0.65em;
    font-weight: 600;
    padding: 0.05em 0.45em;
    border: 0.08em solid currentColor;
    border-radius: 0.4em;
    opacity: 0.85;
  }
  .page {
    position: absolute;
    right: 3%;
    bottom: 3%;
    font-size: 3cqh;
    opacity: 0.6;
  }
  .placeholder {
    position: absolute;
    inset: 0;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 3%;
    text-align: center;
    padding: 0 8%;
    font-size: 6cqh;
  }
  .logo {
    font-size: 4em;
    line-height: 1;
    opacity: 0.8;
  }
  .placeholder span {
    opacity: 0.75;
    font-size: 0.8em;
  }
</style>
