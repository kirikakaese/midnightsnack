<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Chooses what a new OpenSlides cue shows, from the connected meeting. -->
<script lang="ts">
  import type { OpenSlidesSlide } from "@midnightsnack/protocol";
  import { Button, t, type HostConnection } from "@midnightsnack/ui";

  interface Props {
    conn: HostConnection;
    onpick: (slide: OpenSlidesSlide, name: string) => void;
    oncancel: () => void;
  }
  let { conn, onpick, oncancel }: Props = $props();

  const data = $derived(conn.openslides);
  type Kind = "agenda" | "current_speakers" | "follow" | "motion" | "topic" | "speakers";
  let kind = $state<Kind>("agenda");
  let motionId = $state<number | null>(null);
  let topicId = $state<number | null>(null);
  let listId = $state<number | null>(null);
  let projectorId = $state<number | null>(null);

  const needsData = $derived(kind === "motion" || kind === "topic" || kind === "speakers");
  const ready = $derived(
    kind === "motion"
      ? motionId !== null
      : kind === "topic"
        ? topicId !== null
        : kind === "speakers"
          ? listId !== null
          : true,
  );

  function motionLabel(m: { number: string; title: string }) {
    return m.number ? `${m.number} · ${m.title}` : m.title;
  }

  function pick() {
    if (!ready) return;
    switch (kind) {
      case "agenda":
        return onpick({ kind: "agenda" }, t("os.agenda"));
      case "current_speakers":
        return onpick({ kind: "speakers", list_id: null }, t("os.speakers"));
      case "follow": {
        const p = data?.projectors.find((x) => x.id === projectorId);
        return onpick(
          { kind: "follow", projector_id: projectorId },
          p ? t("os.follow_named", { name: p.name }) : t("os.follow"),
        );
      }
      case "motion": {
        const m = data?.motions.find((x) => x.id === motionId);
        return onpick({ kind: "motion", motion_id: motionId! }, m ? motionLabel(m) : "");
      }
      case "topic": {
        const tp = data?.topics.find((x) => x.id === topicId);
        return onpick({ kind: "topic", topic_id: topicId! }, tp?.title ?? "");
      }
      case "speakers": {
        const l = data?.lists.find((x) => x.id === listId);
        return onpick(
          { kind: "speakers", list_id: listId },
          l ? t("os.speakers_of", { title: l.title }) : t("os.speakers"),
        );
      }
    }
  }
</script>

<form
  class="ms-form"
  onsubmit={(e) => {
    e.preventDefault();
    pick();
  }}
>
  {#if !data}
    <p class="hint">{t("os.connect_first")}</p>
  {:else}
    <p class="hint">{t("os.meeting", { name: data.name })}</p>
  {/if}
  <label>
    <span>{t("os.show")}</span>
    <select bind:value={kind}>
      <option value="agenda">{t("os.agenda")}</option>
      <option value="current_speakers">{t("os.current_speakers")}</option>
      <option value="follow">{t("os.follow")}</option>
      <option value="motion" disabled={!data}>{t("os.motion")}</option>
      <option value="topic" disabled={!data}>{t("os.topic")}</option>
      <option value="speakers" disabled={!data}>{t("os.speakers_of_item")}</option>
    </select>
  </label>
  {#if kind === "follow"}
    <label>
      <span>{t("os.projector")}</span>
      <select bind:value={projectorId}>
        <option value={null}>{t("os.reference_projector")}</option>
        {#each data?.projectors ?? [] as p (p.id)}
          <option value={p.id}>{p.name}</option>
        {/each}
      </select>
    </label>
    <p class="hint">{t("os.follow_hint")}</p>
  {:else if kind === "motion" && data}
    <label>
      <span>{t("os.motion")}</span>
      <select bind:value={motionId} required>
        <option value={null} disabled>{t("os.choose")}</option>
        {#each data.motions as m (m.id)}
          <option value={m.id}>{motionLabel(m)}</option>
        {/each}
      </select>
    </label>
  {:else if kind === "topic" && data}
    <label>
      <span>{t("os.topic")}</span>
      <select bind:value={topicId} required>
        <option value={null} disabled>{t("os.choose")}</option>
        {#each data.topics as tp (tp.id)}
          <option value={tp.id}>{tp.title}</option>
        {/each}
      </select>
    </label>
  {:else if kind === "speakers" && data}
    <label>
      <span>{t("os.speakers_of_item")}</span>
      <select bind:value={listId} required>
        <option value={null} disabled>{t("os.choose")}</option>
        {#each data.lists as l (l.id)}
          <option value={l.id}>{l.title || `#${l.id}`}</option>
        {/each}
      </select>
    </label>
  {/if}
  <div class="row">
    <Button variant="go" type="submit" disabled={!ready || (needsData && !data)}>
      {t("cue.add")}
    </Button>
    <Button variant="ghost" type="button" onclick={oncancel}>{t("common.cancel")}</Button>
  </div>
</form>
