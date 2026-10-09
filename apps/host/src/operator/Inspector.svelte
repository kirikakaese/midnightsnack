<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type {
    CaptureSource,
    MediaOptions,
    TextTheme,
    TimerCue,
    TimerMode,
    Transition,
    WebInfo,
  } from "@midnightsnack/protocol";
  import {
    Button,
    captureLabel,
    formatDuration,
    parseDuration,
    t,
    type HostConnection,
  } from "@midnightsnack/ui";
  import { open } from "@tauri-apps/plugin-dialog";
  import CapturePicker from "./CapturePicker.svelte";
  import ThemeEditor from "./ThemeEditor.svelte";

  interface Props {
    conn: HostConnection;
    cueId: string | null;
  }
  let { conn, cueId }: Props = $props();

  const cue = $derived(conn.cue(cueId));
  const show = $derived(conn.show);

  // Drafts for fields that are applied explicitly (text) or on blur.
  let name = $derived(cue?.name ?? "");
  let notes = $derived(cue?.notes ?? "");
  let text = $derived(cue?.text?.source ?? "");
  let lyrics = $derived(cue?.text?.lyrics ?? true);
  const textDirty = $derived(
    !!cue?.text && (text !== cue.text.source || lyrics !== cue.text.lyrics),
  );

  const imageFilter = [
    { name: t("cue.filter_images"), extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp"] },
  ];

  function id(): string {
    return cue!.id;
  }

  function setTransition(value: string, duration: number) {
    const transition: Transition | null =
      value === "default"
        ? null
        : { kind: value === "fade" ? "fade" : "cut", duration_ms: duration };
    conn.action({ action: "set_cue_transition", cue_id: id(), transition });
  }

  function setMedia(patch: Partial<MediaOptions>) {
    if (!cue?.media) return;
    conn.action({
      action: "set_media_options",
      cue_id: id(),
      options: { ...cue.media.options, ...patch },
    });
  }

  function setTimer(patch: Partial<TimerCue>) {
    if (!cue?.timer) return;
    conn.action({ action: "set_cue_timer", cue_id: id(), timer: { ...cue.timer, ...patch } });
  }

  function setTimerMode(mode: string) {
    const modes: Record<string, TimerMode> = {
      countdown: { mode: "countdown", duration_ms: 5 * 60_000 },
      countdown_to: { mode: "countdown_to", time: "19:00" },
      count_up: { mode: "count_up" },
      clock: { mode: "clock" },
    };
    const m = modes[mode];
    if (m) setTimer({ mode: m });
  }

  const ownTheme = $derived(cue?.text?.theme ?? cue?.timer?.theme ?? null);
  function setTheme(theme: TextTheme | null) {
    conn.action({ action: "set_cue_theme", cue_id: id(), theme });
  }

  async function pickBackground() {
    const path = await open({ multiple: false, filters: imageFilter });
    if (typeof path === "string")
      conn.action({ action: "set_background_image", cue_id: id(), path });
  }

  // Outputs that can show cues (stage feeds follow the program on their own).
  const programOutputs = $derived(show?.outputs.filter((o) => o.feed === "program") ?? []);

  function setTargets(targets: string[] | null) {
    conn.action({ action: "set_cue_targets", cue_id: id(), targets });
  }

  function toggleTarget(outputId: string, on: boolean) {
    const current = cue?.targets ?? programOutputs.map((o) => o.id);
    const next = on ? [...new Set([...current, outputId])] : current.filter((t) => t !== outputId);
    setTargets(next);
  }

  let webUrl = $derived(cue?.web?.url ?? "");
  function setWeb(patch: Partial<WebInfo>) {
    if (!cue?.web) return;
    conn.action({ action: "set_web_options", cue_id: id(), web: { ...cue.web, ...patch } });
  }

  let pickingCapture = $state(false);
  function setCapture(source: CaptureSource, fps: number) {
    conn.action({ action: "set_capture", cue_id: id(), source, fps });
    pickingCapture = false;
  }

  function msField(ms: number | null): string {
    return ms === null ? "" : formatDuration(ms);
  }
</script>

{#if !cue || !show}
  <p class="hint">{t("inspector.none")}</p>
{:else}
  <div class="ms-form">
    <label>
      <span>{t("inspector.name")}</span>
      <input
        bind:value={name}
        onblur={() =>
          name.trim() &&
          name !== cue.name &&
          conn.action({ action: "rename_cue", cue_id: cue.id, name })}
      />
    </label>

    {#if cue.text}
      <h3>{t("inspector.text")}</h3>
      <label>
        <span class="ms-visually-hidden">{t("inspector.text")}</span>
        <textarea bind:value={text} rows="8" placeholder={t("inspector.text_placeholder")}
        ></textarea>
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={lyrics} />
        {t("inspector.lyrics")}
      </label>
      <p class="hint">{lyrics ? t("inspector.lyrics_hint") : t("inspector.separator_hint")}</p>
      <div class="row">
        <Button
          variant="go"
          disabled={!textDirty}
          onclick={() => conn.action({ action: "set_cue_text", cue_id: cue.id, text, lyrics })}
        >
          {t("inspector.apply_text")}
        </Button>
        <span class="hint">{t("cue.slides", { n: cue.slide_count })}</span>
      </div>
    {/if}

    {#if cue.timer}
      {@const timer = cue.timer}
      <h3>{t("inspector.timer")}</h3>
      <label>
        <span>{t("inspector.timer_mode")}</span>
        <select value={timer.mode.mode} onchange={(e) => setTimerMode(e.currentTarget.value)}>
          <option value="countdown">{t("timer_mode.countdown")}</option>
          <option value="countdown_to">{t("timer_mode.countdown_to")}</option>
          <option value="count_up">{t("timer_mode.count_up")}</option>
          <option value="clock">{t("timer_mode.clock")}</option>
        </select>
      </label>
      {#if timer.mode.mode === "countdown"}
        <label>
          <span>{t("inspector.duration")}</span>
          <input
            value={formatDuration(timer.mode.duration_ms)}
            onchange={(e) => {
              const ms = parseDuration(e.currentTarget.value);
              if (ms) setTimer({ mode: { mode: "countdown", duration_ms: ms } });
            }}
          />
        </label>
      {:else if timer.mode.mode === "countdown_to"}
        <label>
          <span>{t("inspector.until")}</span>
          <input
            type="time"
            value={timer.mode.time}
            onchange={(e) =>
              setTimer({ mode: { mode: "countdown_to", time: e.currentTarget.value } })}
          />
        </label>
      {/if}
      <label>
        <span>{t("inspector.label")}</span>
        <input value={timer.label} onchange={(e) => setTimer({ label: e.currentTarget.value })} />
      </label>
      <label class="check">
        <input
          type="color"
          value={timer.overtime_color}
          onchange={(e) => setTimer({ overtime_color: e.currentTarget.value })}
        />
        {t("inspector.overtime_color")}
      </label>
    {/if}

    {#if cue.text || cue.timer}
      <h3>{t("inspector.look")}</h3>
      <label class="check">
        <input
          type="checkbox"
          checked={ownTheme !== null}
          onchange={(e) => setTheme(e.currentTarget.checked ? { ...show.default_theme } : null)}
        />
        {t("inspector.own_theme")}
      </label>
      {#if ownTheme}
        <ThemeEditor
          theme={ownTheme}
          onchange={setTheme}
          onpickimage={pickBackground}
          onclearimage={() =>
            conn.action({ action: "set_background_image", cue_id: cue.id, path: null })}
        />
      {/if}
    {/if}

    {#if cue.media}
      {@const m = cue.media.options}
      <h3>{t("inspector.playback")}</h3>
      <label class="check">
        <input
          type="checkbox"
          checked={m.loop}
          onchange={(e) => setMedia({ loop: e.currentTarget.checked })}
        />
        {t("inspector.loop")}
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={m.auto_advance}
          disabled={m.loop}
          onchange={(e) => setMedia({ auto_advance: e.currentTarget.checked })}
        />
        {t("inspector.advance_at_end")}
      </label>
      <div class="row">
        <label>
          <span>{t("inspector.start")}</span>
          <input
            value={msField(m.start_ms)}
            size="7"
            onchange={(e) =>
              setMedia({
                start_ms: e.currentTarget.value.trim()
                  ? (parseDuration(e.currentTarget.value) ?? 0)
                  : 0,
              })}
          />
        </label>
        <label>
          <span>{t("inspector.end")}</span>
          <input
            value={msField(m.end_ms)}
            size="7"
            placeholder={cue.media.duration_ms ? formatDuration(cue.media.duration_ms) : ""}
            onchange={(e) => setMedia({ end_ms: parseDuration(e.currentTarget.value) })}
          />
        </label>
      </div>
      <label>
        <span>{t("inspector.volume", { n: Math.round(m.volume * 100) })}</span>
        <input
          type="range"
          min="0"
          max="100"
          value={Math.round(m.volume * 100)}
          onchange={(e) => setMedia({ volume: Number(e.currentTarget.value) / 100 })}
        />
      </label>
    {/if}

    {#if cue.web}
      {@const web = cue.web}
      <h3>{web.openslides ? t("cue.kind.openslides") : t("inspector.web")}</h3>
      <label>
        <span>{t("cue.web_url")}</span>
        <input
          type="url"
          bind:value={webUrl}
          onchange={() => webUrl.trim() !== web.url && setWeb({ url: webUrl.trim() })}
        />
      </label>
      <label>
        <span>{t("inspector.zoom", { n: web.zoom })}</span>
        <input
          type="range"
          min="25"
          max="400"
          step="5"
          value={web.zoom}
          onchange={(e) => setWeb({ zoom: Number(e.currentTarget.value) })}
        />
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={web.block_navigation}
          onchange={(e) => setWeb({ block_navigation: e.currentTarget.checked })}
        />
        {t("inspector.block_navigation")}
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={web.forward_keys}
          onchange={(e) => setWeb({ forward_keys: e.currentTarget.checked })}
        />
        {t("inspector.forward_keys")}
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={web.persist_session}
          onchange={(e) => setWeb({ persist_session: e.currentTarget.checked })}
        />
        {t("inspector.persist_session")}
      </label>
      <p class="hint">
        {web.persist_session ? t("inspector.persist_hint") : t("inspector.private_hint")}
      </p>
    {/if}

    {#if cue.capture}
      {@const capture = cue.capture}
      <h3>{t("inspector.capture")}</h3>
      <p>
        {t(`capture.kind_${capture.source.type}`)}: <strong>{captureLabel(capture.source)}</strong>
      </p>
      {#if conn.live?.capture_lost.includes(cue.id)}
        <p class="warn" role="alert">{t("inspector.capture_lost")}</p>
      {/if}
      {#if pickingCapture}
        <CapturePicker
          {conn}
          onpick={(source) => setCapture(source, capture.fps)}
          oncancel={() => (pickingCapture = false)}
        />
      {:else}
        <Button onclick={() => (pickingCapture = true)}>{t("inspector.change_source")}</Button>
      {/if}
      <label>
        <span>{t("inspector.fps")}</span>
        <select
          value={String(capture.fps)}
          onchange={(e) => setCapture(capture.source, Number(e.currentTarget.value))}
        >
          {#each [5, 10, 15, 24, 30] as fps (fps)}
            <option value={String(fps)}>{fps}</option>
          {/each}
        </select>
      </label>
    {/if}

    {#if programOutputs.length > 1}
      <h3>{t("inspector.outputs")}</h3>
      <label class="check">
        <input
          type="checkbox"
          checked={cue.targets === null}
          onchange={(e) =>
            setTargets(e.currentTarget.checked ? null : programOutputs.map((o) => o.id))}
        />
        {t("inspector.all_outputs")}
      </label>
      {#if cue.targets !== null}
        {#each programOutputs as o (o.id)}
          <label class="check">
            <input
              type="checkbox"
              checked={cue.targets.includes(o.id)}
              onchange={(e) => toggleTarget(o.id, e.currentTarget.checked)}
            />
            {o.name}
          </label>
        {/each}
        <p class="hint">{t("inspector.targets_hint")}</p>
      {/if}
    {/if}

    <h3>{t("inspector.transition")}</h3>
    <div class="row">
      <select
        value={cue.transition ? cue.transition.kind : "default"}
        aria-label={t("inspector.transition")}
        onchange={(e) => setTransition(e.currentTarget.value, cue.transition?.duration_ms ?? 400)}
      >
        <option value="default">{t("transition.default")}</option>
        <option value="cut">{t("transition.cut")}</option>
        <option value="fade">{t("transition.fade")}</option>
      </select>
      {#if cue.transition?.kind === "fade"}
        <label>
          <span>{t("inspector.duration_ms")}</span>
          <input
            type="number"
            min="0"
            max="10000"
            step="50"
            value={cue.transition.duration_ms}
            onchange={(e) =>
              setTransition(
                "fade",
                Math.max(0, Math.min(10000, Number(e.currentTarget.value) || 0)),
              )}
          />
        </label>
      {/if}
    </div>

    <h3>{t("inspector.auto_advance")}</h3>
    <div class="row">
      <label class="check">
        <input
          type="checkbox"
          checked={cue.auto_advance_ms !== null}
          onchange={(e) =>
            conn.action({
              action: "set_cue_auto_advance",
              cue_id: cue.id,
              after_ms: e.currentTarget.checked ? 5000 : null,
            })}
        />
        {t("inspector.advance_after")}
      </label>
      {#if cue.auto_advance_ms !== null}
        <input
          type="number"
          min="0.1"
          step="0.5"
          value={cue.auto_advance_ms / 1000}
          aria-label={t("inspector.seconds")}
          onchange={(e) => {
            const s = Number(e.currentTarget.value);
            if (s >= 0.1)
              conn.action({
                action: "set_cue_auto_advance",
                cue_id: cue.id,
                after_ms: Math.round(s * 1000),
              });
          }}
        />
        <span>{t("inspector.seconds")}</span>
      {/if}
    </div>

    <h3>{t("inspector.notes")}</h3>
    <label>
      <span class="ms-visually-hidden">{t("inspector.notes")}</span>
      <textarea
        bind:value={notes}
        rows="4"
        onblur={() =>
          notes !== cue.notes && conn.action({ action: "set_cue_notes", cue_id: cue.id, notes })}
      ></textarea>
    </label>
  </div>
{/if}

<style>
  .hint {
    color: var(--ms-text-muted);
  }
  .warn {
    color: var(--ms-warn);
  }
</style>
