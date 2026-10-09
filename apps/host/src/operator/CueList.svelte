<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { CaptureSource, CueSummary } from "@midnightsnack/protocol";
  import { Button, Panel, captureLabel, t, type HostConnection } from "@midnightsnack/ui";
  import { open } from "@tauri-apps/plugin-dialog";
  import CapturePicker from "./CapturePicker.svelte";
  import InboxPanel from "./InboxPanel.svelte";

  interface Props {
    conn: HostConnection;
    /** Selected cue (shown in the inspector). */
    selected: string | null;
  }
  let { conn, selected = $bindable() }: Props = $props();

  const cues = $derived(conn.show?.cues ?? []);
  const programCue = $derived(conn.live?.program?.cue_id);
  const outputCue = $derived(conn.live?.output?.cue_id);
  const nextCue = $derived(conn.live?.next?.cue_id);
  let editing = $state<string | null>(null);
  let editName = $state("");
  let dragId = $state<string | null>(null);

  const COLORS = [null, "#ef4444", "#f59e0b", "#22c55e", "#38bdf8", "#a78bfa", "#ec4899"];

  const mediaFilter = [
    {
      name: t("cue.filter_media"),
      extensions: [
        "pdf",
        "pptx",
        "ppt",
        "odp",
        "key",
        "png",
        "jpg",
        "jpeg",
        "gif",
        "webp",
        "bmp",
        "tif",
        "tiff",
        "mp4",
        "m4v",
        "mov",
        "webm",
        "mkv",
        "ogv",
        "mp3",
        "m4a",
        "aac",
        "wav",
        "ogg",
        "oga",
        "opus",
        "flac",
      ],
    },
  ];

  // After adding a cue, select it so the inspector shows it.
  let known: Set<string> | null = null;
  function expectNewCue() {
    known = new Set(cues.map((c) => c.id));
  }
  $effect(() => {
    const ids = cues.map((c) => c.id);
    if (!known) return;
    const added = ids.find((id) => !known!.has(id));
    if (added) {
      selected = added;
      known = null;
    }
  });

  function addText() {
    expectNewCue();
    conn.action({
      action: "add_text",
      name: t("cue.new_text_name"),
      text: t("cue.new_text_body"),
      lyrics: true,
      at_index: null,
    });
  }

  function addTimer() {
    expectNewCue();
    conn.action({
      action: "add_timer",
      name: t("cue.new_timer_name"),
      timer: {
        mode: { mode: "countdown", duration_ms: 5 * 60_000 },
        label: "",
        overtime_color: "#ef4444",
        theme: null,
      },
      at_index: null,
    });
  }

  // Inline forms for cues that need a URL or a capture source first.
  let adding = $state<"web" | "openslides" | "capture" | null>(null);
  let webUrl = $state("https://");
  const webUrlValid = $derived(/^https?:\/\/[^\s/?#]+/i.test(webUrl.trim()));

  function startAdding(kind: "web" | "openslides" | "capture") {
    adding = kind;
    webUrl = "https://";
  }

  function addWeb() {
    if (!webUrlValid) return;
    const openslides = adding === "openslides";
    const url = webUrl.trim();
    expectNewCue();
    conn.action({
      action: "add_web",
      name: openslides ? t("cue.kind.openslides") : hostName(url),
      web: {
        url,
        zoom: 100,
        // OpenSlides projector pages update themselves; nothing should navigate away.
        block_navigation: true,
        forward_keys: !openslides,
        persist_session: openslides,
        openslides,
      },
      at_index: null,
    });
    adding = null;
  }

  function hostName(url: string): string {
    try {
      return new URL(url).hostname;
    } catch {
      return t("cue.kind.web");
    }
  }

  function addCapture(source: CaptureSource) {
    expectNewCue();
    conn.action({ action: "add_capture", name: captureLabel(source), source, at_index: null });
    adding = null;
  }

  function meta(cue: CueSummary): string {
    if (cue.web)
      return `${t(cue.web.openslides ? "cue.kind.openslides" : "cue.kind.web")} · ${hostName(cue.web.url)}`;
    if (cue.capture) return t("cue.kind.capture");
    const kind = t(`cue.kind.${cue.kind}`);
    const from = cue.converted_from ? ` · ${cue.converted_from}` : "";
    return `${kind} · ${t("cue.slides", { n: cue.slide_count })}${from}`;
  }

  function addMore(kind: string) {
    if (kind === "folder") addFolder();
    else if (kind === "text") addText();
    else if (kind === "timer") addTimer();
    else if (kind === "blank")
      conn.action({ action: "add_blank", color: "#000000", at_index: null });
    else if (kind === "web" || kind === "openslides" || kind === "capture") startAdding(kind);
  }

  async function addFiles() {
    expectNewCue();
    const paths = await open({ multiple: true, filters: mediaFilter });
    if (paths?.length) conn.action({ action: "add_files", paths, at_index: null });
  }

  async function addFolder() {
    const path = await open({ directory: true });
    if (typeof path === "string")
      conn.action({ action: "add_files", paths: [path], at_index: null });
  }

  function go(cue: CueSummary) {
    if (cue.slide_count > 0)
      conn.action({ action: "go_to", position: { cue_id: cue.id, slide: 0 } });
  }

  function startRename(cue: CueSummary) {
    editing = cue.id;
    editName = cue.name;
  }

  function commitRename(cue: CueSummary) {
    if (editing !== cue.id) return;
    editing = null;
    const name = editName.trim();
    if (name && name !== cue.name) conn.action({ action: "rename_cue", cue_id: cue.id, name });
  }

  function move(cue: CueSummary, delta: number) {
    const idx = cues.findIndex((c) => c.id === cue.id);
    const to = idx + delta;
    if (to < 0 || to >= cues.length) return;
    conn.action({ action: "move_cue", cue_id: cue.id, to_index: to });
  }

  function cycleColor(cue: CueSummary) {
    const i = COLORS.indexOf(cue.color);
    const color = COLORS[(i + 1) % COLORS.length] ?? null;
    conn.action({ action: "set_cue_color", cue_id: cue.id, color });
  }

  function onDrop(target: CueSummary) {
    if (!dragId || dragId === target.id) return;
    const to = cues.findIndex((c) => c.id === target.id);
    conn.action({ action: "move_cue", cue_id: dragId, to_index: to });
    dragId = null;
  }
</script>

<Panel title={t("cue.list")}>
  {#snippet actions()}
    <Button onclick={addFiles}>{t("cue.add_files")}</Button>
    <select
      class="add-menu"
      aria-label={t("cue.add_more")}
      value=""
      onchange={(e) => {
        const v = e.currentTarget.value;
        e.currentTarget.value = "";
        addMore(v);
      }}
    >
      <option value="" disabled>{t("cue.add_more")}</option>
      <option value="folder">{t("cue.add_folder")}</option>
      <option value="text">{t("cue.add_text")}</option>
      <option value="timer">{t("cue.add_timer")}</option>
      <option value="blank">{t("cue.add_blank")}</option>
      <option value="web">{t("cue.add_web")}</option>
      <option value="openslides">{t("cue.add_openslides")}</option>
      <option value="capture">{t("cue.add_capture")}</option>
    </select>
  {/snippet}

  {#if adding === "web" || adding === "openslides"}
    <form
      class="add-form"
      onsubmit={(e) => {
        e.preventDefault();
        addWeb();
      }}
    >
      <label>
        <span>{adding === "openslides" ? t("cue.openslides_url") : t("cue.web_url")}</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input type="url" bind:value={webUrl} autofocus required />
      </label>
      {#if adding === "openslides"}<p class="hint">{t("cue.openslides_hint")}</p>{/if}
      <div class="row">
        <Button variant="go" type="submit" disabled={!webUrlValid}>{t("cue.add")}</Button>
        <Button variant="ghost" type="button" onclick={() => (adding = null)}>
          {t("common.cancel")}
        </Button>
      </div>
    </form>
  {:else if adding === "capture"}
    <div class="add-form">
      <CapturePicker {conn} onpick={addCapture} oncancel={() => (adding = null)} />
    </div>
  {/if}

  <InboxPanel {conn} />

  {#if cues.length === 0}
    <p class="empty">{t("cue.empty")}</p>
  {:else}
    <ol class="cues" aria-label={t("cue.list")}>
      {#each cues as cue, i (cue.id)}
        <li
          class="cue"
          class:program={cue.id === programCue}
          class:output={cue.id === outputCue}
          class:next={cue.id === nextCue && cue.id !== programCue}
          class:selected={cue.id === selected}
          draggable="true"
          ondragstart={() => (dragId = cue.id)}
          ondragover={(e) => e.preventDefault()}
          ondrop={() => onDrop(cue)}
        >
          <button
            class="tag"
            style:background={cue.color ?? "transparent"}
            aria-label={t("cue.color")}
            onclick={() => cycleColor(cue)}
          ></button>
          <span class="num">{i + 1}</span>
          {#if editing === cue.id}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="rename"
              bind:value={editName}
              autofocus
              aria-label={t("cue.rename")}
              onblur={() => commitRename(cue)}
              onkeydown={(e) => {
                if (e.key === "Enter") commitRename(cue);
                if (e.key === "Escape") editing = null;
              }}
            />
          {:else}
            <button
              class="name"
              aria-pressed={cue.id === selected}
              onclick={() => (selected = cue.id)}
              ondblclick={() => go(cue)}
            >
              <span class="title">{cue.name || t("cue.unnamed")}</span>
              <span class="meta">
                {meta(cue)}
                {#if cue.targets}
                  · <span class="targets">{t("cue.targets_n", { n: cue.targets.length })}</span>
                {/if}
              </span>
            </button>
          {/if}
          <span class="tools">
            <button
              class="go"
              aria-label={t("cue.go_live")}
              disabled={cue.slide_count === 0}
              onclick={() => go(cue)}>▶</button
            >
            <button aria-label={t("cue.rename")} onclick={() => startRename(cue)}>✎</button>
            <button aria-label={t("cue.move_up")} disabled={i === 0} onclick={() => move(cue, -1)}
              >↑</button
            >
            <button
              aria-label={t("cue.move_down")}
              disabled={i === cues.length - 1}
              onclick={() => move(cue, 1)}>↓</button
            >
            <button
              aria-label={t("cue.remove")}
              onclick={() => conn.action({ action: "remove_cue", cue_id: cue.id })}>✕</button
            >
          </span>
        </li>
      {/each}
    </ol>
  {/if}
</Panel>

<style>
  .add-menu {
    min-height: 40px;
    padding: 0 10px;
    font: inherit;
    font-weight: 600;
    color: var(--ms-text);
    background: var(--ms-surface-2);
    border: 1px solid var(--ms-border);
    border-radius: var(--ms-radius-sm);
    cursor: pointer;
  }
  .add-form {
    display: grid;
    gap: 6px;
    margin-bottom: 8px;
  }
  .add-form label {
    display: grid;
    gap: 4px;
  }
  .add-form .row {
    display: flex;
    gap: 6px;
  }
  .hint {
    margin: 0;
    font-size: 0.8rem;
    color: var(--ms-text-muted);
  }
  .targets {
    color: var(--ms-accent);
  }
  .empty {
    color: var(--ms-text-muted);
  }
  .cues {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
  }
  .cue {
    display: grid;
    grid-template-columns: 10px 28px 1fr auto;
    align-items: center;
    gap: 6px;
    border: 2px solid transparent;
    border-radius: var(--ms-radius-sm);
    background: var(--ms-surface-2);
    padding: 2px 4px 2px 0;
  }
  .cue.next {
    border-color: var(--ms-accent);
  }
  .cue.program {
    border-color: var(--ms-go);
  }
  .cue.selected {
    background: var(--ms-surface-3);
  }
  .tools .go {
    color: var(--ms-go);
  }
  .cue.output:not(.program) {
    border-color: var(--ms-freeze);
  }
  .tag {
    width: 10px;
    height: 100%;
    min-height: 40px;
    border: 0;
    border-radius: 4px 0 0 4px;
    cursor: pointer;
    padding: 0;
  }
  .num {
    color: var(--ms-text-muted);
    font-family: var(--ms-font-mono);
    text-align: right;
  }
  .name {
    display: grid;
    text-align: left;
    background: none;
    border: 0;
    color: inherit;
    font: inherit;
    padding: 6px 0;
    cursor: pointer;
    min-width: 0;
  }
  .title {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    font-size: 0.8rem;
    color: var(--ms-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rename {
    font: inherit;
    padding: 6px;
    background: var(--ms-bg);
    color: var(--ms-text);
    border: 1px solid var(--ms-accent);
    border-radius: 4px;
  }
  .tools {
    display: flex;
    gap: 2px;
  }
  /* Row tools only take room on the row in use, so names stay readable in narrow lists. */
  .tools button:not(.go) {
    display: none;
  }
  .cue:hover .tools button,
  .cue:focus-within .tools button,
  .cue.selected .tools button {
    display: inline-block;
  }
  .tools button {
    min-width: 30px;
    min-height: 30px;
    background: transparent;
    border: 0;
    color: var(--ms-text-muted);
    border-radius: 4px;
    cursor: pointer;
  }
  .tools button:hover:not(:disabled) {
    background: var(--ms-surface-3);
    color: var(--ms-text);
  }
  .tools button:disabled {
    opacity: 0.3;
  }
</style>
