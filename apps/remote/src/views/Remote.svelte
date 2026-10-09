<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { Action, ErrorCode, Role } from "@midnightsnack/protocol";
  import {
    Button,
    HostConnection,
    Panel,
    PointerPad,
    Stage,
    StageDisplay,
    StatusDot,
    Ticker,
    elapsedMs,
    formatDuration,
    mediaPosition,
    t,
  } from "@midnightsnack/ui";
  import { onDestroy, untrack } from "svelte";
  import {
    makeTransport,
    storage,
    switchToLan,
    switchToRelay,
    type HostLink,
  } from "../lib/storage";
  import { keepScreenOn, tap } from "../lib/wakelock";

  interface Props {
    link: HostLink;
    token: string;
    onunpaired: () => void;
  }
  let { link, token, onunpaired }: Props = $props();

  // The parent remounts this view (`{#key}`) when the token changes, and only shows it for
  // usable links.
  const conn = new HostConnection({
    transport: makeTransport(untrack(() => link))!,
    token: untrack(() => token),
  });
  conn.connect();
  const viaRelay = conn.transport.kind === "relay";
  const ticker = new Ticker(500);
  const releaseWake = keepScreenOn();
  let view = $state<"control" | "stage">("control");

  onDestroy(() => {
    conn.close();
    ticker.stop();
    releaseWake();
  });

  $effect(() => {
    if (conn.status === "unauthorized") storage.setToken(link, null);
  });

  // Fallback: when the local network stays unreachable and the host has a relay, move there
  // (after a short countdown the user can cancel); on the relay, offer the way back.
  const FALLBACK_AFTER_MS = 8000;
  const FALLBACK_COUNTDOWN_MS = 5000;
  let knownRoutes = $state(storage.routes());
  $effect(() => {
    if (conn.routes && !viaRelay) {
      storage.setRoutes(conn.routes);
      knownRoutes = conn.routes;
    }
  });
  const routes = $derived(conn.routes ?? (viaRelay ? null : knownRoutes));
  let lostSince = $state<number | null>(null);
  let stayLocal = $state(false);
  $effect(() => {
    if (conn.status === "connected") {
      lostSince = null;
      stayLocal = false;
    } else if (conn.status === "disconnected" || conn.status === "connecting") {
      lostSince ??= Date.now();
    }
  });
  const relayRoute = $derived(routes?.relay ?? null);
  const fallbackIn = $derived(
    !viaRelay && relayRoute && lostSince !== null && !stayLocal
      ? FALLBACK_AFTER_MS + FALLBACK_COUNTDOWN_MS - (ticker.now - lostSince)
      : null,
  );
  $effect(() => {
    if (fallbackIn !== null && fallbackIn <= 0 && relayRoute) {
      switchToRelay(
        relayRoute,
        untrack(() => token),
      );
    }
  });
  const lanBase = $derived(viaRelay ? (conn.routes?.lan[0] ?? null) : null);

  const rank: Record<Role, number> = { stage_viewer: 0, presenter: 1, operator: 2, admin: 3 };
  const role = $derived(conn.session?.role ?? "stage_viewer");
  const can = (min: Role) => rank[role] >= rank[min];
  const showStage = $derived(view === "stage" || role === "stage_viewer");

  const live = $derived(conn.live);
  const programCue = $derived(conn.cue(live?.program?.cue_id));
  const nextCue = $derived(conn.cue(live?.next?.cue_id));
  const notes = $derived(
    programCue && live?.program ? (programCue.slide_notes[live.program.slide] ?? "") : "",
  );
  const hostNow = $derived(ticker.now + conn.clockOffset);
  const showTime = $derived(formatDuration(elapsedMs(live?.show_timer, hostNow)));
  const slideTime = $derived(formatDuration(elapsedMs(live?.slide_timer, hostNow)));
  const statusKind = $derived(
    conn.status === "connected" ? "ok" : conn.status === "connecting" ? "pending" : "error",
  );

  const media = $derived(live?.media ?? null);
  const mediaCue = $derived(conn.cue(media?.cue_id));
  const mediaPlaying = $derived(
    !!media && media.position.running_since_ms !== null && !media.ended,
  );
  const mediaTime = $derived(
    media && mediaCue?.media
      ? formatDuration(
          mediaPosition(
            media.position,
            mediaCue.media.options,
            mediaCue.media.duration_ms,
            hostNow,
          ),
        )
      : "",
  );
  const overlays = $derived(conn.show?.overlays ?? []);
  const countdownRunning = $derived(live?.countdown.elapsed.running_since_ms != null);
  const countdownLeft = $derived(
    live ? live.countdown.duration_ms - elapsedMs(live.countdown.elapsed, hostNow) : 0,
  );
  let message = $state("");

  // Laser pointer and drawing on the current slide (presenters and up).
  const POINTER_COLORS = ["#ff3b30", "#ffd60a", "#30d158", "#0a84ff", "#ffffff"];
  let pointerMode = $state<"off" | "point" | "draw">("off");
  let pointerColor = $state(POINTER_COLORS[0]!);
  const hasDrawing = $derived(!!live?.drawing?.strokes.length);
  // Sending files to the host's inbox.
  const UPLOAD_ACCEPT =
    ".pdf,.pptx,.ppt,.pps,.ppsx,.odp,.key,.png,.jpg,.jpeg,.gif,.webp,.bmp,.tif,.tiff," +
    ".mp4,.m4v,.mov,.webm,.mkv,.ogv,.mp3,.m4a,.aac,.wav,.ogg,.oga,.opus,.flac";
  let fileInput = $state<HTMLInputElement>();
  let uploadProgress = $state<number | null>(null);
  let uploadResult = $state<{ ok: boolean; text: string } | null>(null);
  async function sendFile() {
    const file = fileInput?.files?.[0];
    if (!file) return;
    uploadResult = null;
    uploadProgress = 0;
    try {
      const res = await conn.upload(file, (f) => (uploadProgress = f));
      uploadResult = {
        ok: true,
        text: res.added ? t("upload.added", { name: file.name }) : t("upload.waiting"),
      };
    } catch (code) {
      uploadResult = { ok: false, text: t(`error.${code as ErrorCode}`) };
    } finally {
      uploadProgress = null;
      fileInput!.value = "";
    }
  }

  function togglePointer(mode: "point" | "draw") {
    tap();
    pointerMode = pointerMode === mode ? "off" : mode;
  }

  function send(action: Action) {
    tap();
    conn.action(action);
  }
</script>

{#if conn.status === "unauthorized"}
  <main class="message">
    <p>{t("remote.unpaired")}</p>
    <p>{t("remote.scan_again")}</p>
    <Button size="lg" onclick={onunpaired}>{t("remote.pair_again")}</Button>
  </main>
{:else if conn.status === "incompatible"}
  <main class="message"><p>{t("remote.incompatible")}</p></main>
{:else}
  <div class="remote" class:stage={showStage}>
    <header>
      <StatusDot status={statusKind} label={conn.host?.name ?? t("status.connecting")} />
      <span class="role">{t(`role.${role}`)}</span>
      {#if viaRelay}
        <span class="via" title={t("remote.via_relay_hint")}>{t("remote.via_relay")}</span>
        {#if lanBase}
          <button class="switch" onclick={() => switchToLan(lanBase, token)}
            >{t("remote.use_local_network")}</button
          >
        {/if}
      {/if}
      {#if role !== "stage_viewer"}
        <button class="switch" onclick={() => (view = view === "stage" ? "control" : "stage")}>
          {view === "stage" ? t("remote.control_view") : t("remote.stage_view")}
        </button>
      {/if}
    </header>

    {#if fallbackIn !== null && fallbackIn <= FALLBACK_COUNTDOWN_MS && relayRoute}
      <div class="banner fallback" role="alert">
        <span
          >{t("remote.switching_to_relay", { s: Math.max(1, Math.ceil(fallbackIn / 1000)) })}</span
        >
        <Button onclick={() => switchToRelay(relayRoute, token)}>{t("remote.switch_now")}</Button>
        <Button variant="ghost" onclick={() => (stayLocal = true)}>{t("remote.stay_local")}</Button>
      </div>
    {:else if conn.status !== "connected"}
      <div class="banner" role="status">
        {viaRelay && conn.failure ? t(`error.${conn.failure}`) : t("remote.reconnecting")}
      </div>
    {/if}

    {#if showStage}
      <StageDisplay {conn} />
    {:else}
      <section class="screens" class:pointing={pointerMode !== "off"}>
        <figure>
          <div class="screen">
            <PointerPad {conn} mode={pointerMode} color={pointerColor}>
              <Stage {conn} mode="thumb" />
            </PointerPad>
          </div>
          <figcaption>
            {t("remote.current")}
            {#if programCue && live?.program}
              · {programCue.name} · {t("remote.slide_of", {
                n: live.program.slide + 1,
                total: programCue.slide_count,
              })}
            {/if}
          </figcaption>
        </figure>
        <figure class="next">
          <div class="screen"><Stage {conn} which="next" masters={false} mode="thumb" /></div>
          <figcaption>
            {t("remote.next")}{#if nextCue}&nbsp;· {nextCue.name}{/if}
          </figcaption>
        </figure>
      </section>

      {#if can("presenter")}
        <section class="pointer-bar" aria-label={t("pointer.title")}>
          <Button
            variant="go"
            active={pointerMode === "point"}
            aria-pressed={pointerMode === "point"}
            onclick={() => togglePointer("point")}>{t("pointer.laser")}</Button
          >
          <Button
            variant="go"
            active={pointerMode === "draw"}
            aria-pressed={pointerMode === "draw"}
            onclick={() => togglePointer("draw")}>{t("pointer.draw")}</Button
          >
          {#if pointerMode !== "off"}
            <span class="swatches" role="radiogroup" aria-label={t("pointer.color")}>
              {#each POINTER_COLORS as c (c)}
                <button
                  class="swatch"
                  role="radio"
                  aria-checked={pointerColor === c}
                  aria-label={c}
                  style:background={c}
                  onclick={() => (pointerColor = c)}
                ></button>
              {/each}
            </span>
          {/if}
          <Button disabled={!hasDrawing} onclick={() => send({ action: "clear_drawing" })}
            >{t("pointer.clear")}</Button
          >
        </section>
        {#if pointerMode !== "off"}
          <p class="pointer-hint">{t("pointer.hint")}</p>
        {/if}
      {/if}

      {#if can("presenter")}
        <section class="upload" aria-label={t("upload.title")}>
          <input
            bind:this={fileInput}
            type="file"
            accept={UPLOAD_ACCEPT}
            class="ms-visually-hidden"
            id="upload-file"
            onchange={sendFile}
          />
          <Button
            disabled={uploadProgress !== null}
            onclick={() => {
              tap();
              fileInput?.click();
            }}>{t("upload.send")}</Button
          >
          {#if uploadProgress !== null}
            <progress max="1" value={uploadProgress} aria-label={t("upload.progress")}></progress>
          {:else if uploadResult}
            <span class:error={!uploadResult.ok} role="status">{uploadResult.text}</span>
          {/if}
        </section>
      {/if}

      <section class="timers" aria-label={t("timer.show")}>
        <div><span>{t("timer.show")}</span><strong>{showTime}</strong></div>
        <div><span>{t("timer.slide")}</span><strong>{slideTime}</strong></div>
      </section>

      {#if notes}
        <Panel title={t("remote.notes")}>
          <p class="notes">{notes}</p>
        </Panel>
      {/if}
    {/if}

    {#if !showStage}
      {#if can("operator")}
        <section class="masters">
          <Button
            size="lg"
            variant="danger"
            active={live?.masters.blackout}
            onclick={() => send({ action: "toggle_blackout" })}
          >
            {t("controls.blackout")}
          </Button>
          <Button
            size="lg"
            variant="freeze"
            active={live?.masters.freeze}
            onclick={() => send({ action: "toggle_freeze" })}
          >
            {t("controls.freeze")}
          </Button>
          <Button
            size="lg"
            variant="logo"
            active={live?.masters.logo}
            onclick={() => send({ action: "toggle_logo" })}
          >
            {t("controls.logo")}
          </Button>
        </section>

        {#if media && mediaCue}
          <section class="row-panel" aria-label={t("media.transport")}>
            <span class="label">{mediaCue.name} · {mediaTime}</span>
            {#if mediaPlaying}
              <Button onclick={() => send({ action: "media_pause" })}>{t("media.pause")}</Button>
            {:else}
              <Button variant="go" onclick={() => send({ action: "media_play" })}
                >{t("media.play")}</Button
              >
            {/if}
            <Button onclick={() => send({ action: "media_restart" })}>{t("media.restart")}</Button>
          </section>
        {/if}

        {#if overlays.length}
          <Panel title={t("overlay.title")}>
            <div class="chips">
              {#each overlays as o (o.id)}
                <Button
                  variant="go"
                  active={live?.overlays_visible.includes(o.id)}
                  onclick={() => send({ action: "toggle_overlay", overlay_id: o.id })}
                >
                  {o.name || t(`overlay.kind.${o.kind.type}`)}
                </Button>
              {/each}
            </div>
          </Panel>
        {/if}

        <Panel title={t("countdown.title")}>
          <div class="row-panel">
            <strong class="countdown" class:over={countdownLeft < 0}>
              {countdownLeft < 0 ? "+" : ""}{formatDuration(
                Math.abs(countdownLeft) + (countdownLeft > 0 ? 999 : 0),
              )}
            </strong>
            {#if countdownRunning}
              <Button onclick={() => send({ action: "countdown_pause" })}>{t("timer.pause")}</Button
              >
            {:else}
              <Button variant="go" onclick={() => send({ action: "countdown_start" })}
                >{t("timer.start")}</Button
              >
            {/if}
            <Button onclick={() => send({ action: "countdown_reset" })}>{t("timer.reset")}</Button>
          </div>
          <form
            class="row-panel message-form"
            onsubmit={(e) => {
              e.preventDefault();
              send({ action: "set_stage_message", text: message });
              message = "";
            }}
          >
            <input
              bind:value={message}
              placeholder={t("stage_message.placeholder")}
              aria-label={t("stage_message.title")}
              maxlength="500"
            />
            <Button type="submit" disabled={!message.trim()}>{t("stage_message.send")}</Button>
            {#if live?.stage_message}
              <Button
                variant="ghost"
                onclick={() => send({ action: "set_stage_message", text: null })}
                >{t("stage_message.clear")}</Button
              >
            {/if}
          </form>
        </Panel>

        <Panel title={t("remote.cues")}>
          <ol class="cues">
            {#each conn.show?.cues ?? [] as cue (cue.id)}
              <li>
                <button
                  class:live={cue.id === live?.program?.cue_id}
                  disabled={cue.slide_count === 0}
                  onclick={() => send({ action: "go_to", position: { cue_id: cue.id, slide: 0 } })}
                >
                  <span class="tag" style:background={cue.color ?? "transparent"}></span>
                  {cue.name}
                </button>
              </li>
            {/each}
          </ol>
        </Panel>
      {/if}

      {#if can("presenter")}
        <nav class="nav" aria-label={t("controls.label")}>
          <Button size="xl" onclick={() => send({ action: "prev" })}>{t("controls.prev")}</Button>
          <Button size="xl" variant="go" onclick={() => send({ action: "next" })}
            >{t("controls.next")}</Button
          >
        </nav>
      {/if}
    {/if}

    {#if conn.lastError}
      <div class="toast" role="alert">{t(`error.${conn.lastError}`)}</div>
    {/if}
  </div>
{/if}

<style>
  .remote {
    display: grid;
    gap: var(--ms-gap);
    padding: max(10px, env(safe-area-inset-top)) var(--ms-gap)
      calc(150px + env(safe-area-inset-bottom));
    max-width: 900px;
    margin: 0 auto;
  }
  .remote.stage {
    padding-bottom: var(--ms-gap);
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .role {
    margin-left: auto;
    color: var(--ms-text-muted);
    font-size: 0.85rem;
  }
  .switch {
    font: inherit;
    font-size: 0.85rem;
    padding: 6px 10px;
    background: var(--ms-surface-2);
    color: var(--ms-text);
    border: 1px solid var(--ms-border);
    border-radius: var(--ms-radius-sm);
  }
  .banner {
    padding: 8px 12px;
    background: var(--ms-warn);
    color: #241500;
    border-radius: var(--ms-radius-sm);
    font-weight: 600;
  }
  .banner.fallback {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .banner.fallback span {
    flex: 1 1 12em;
  }
  .via {
    font-size: 0.75rem;
    padding: 2px 6px;
    border-radius: 999px;
    border: 1px solid var(--ms-border);
    color: var(--ms-text-muted);
  }
  .screens {
    display: grid;
    grid-template-columns: 2fr 1fr;
    gap: 8px;
    align-items: start;
  }
  /* While pointing, the current slide takes the full width. */
  .screens.pointing {
    grid-template-columns: 1fr;
  }
  .screens.pointing .next {
    display: none;
  }
  .pointer-bar {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .swatches {
    display: flex;
    gap: 6px;
  }
  .swatch {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    border: 2px solid var(--ms-border);
    cursor: pointer;
  }
  .swatch[aria-checked="true"] {
    border-color: var(--ms-text);
    box-shadow: 0 0 0 2px var(--ms-bg) inset;
  }
  .upload {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: 0.85rem;
    color: var(--ms-text-muted);
  }
  .upload progress {
    flex: 1;
  }
  .upload .error {
    color: var(--ms-danger);
  }
  .pointer-hint {
    margin: 0;
    font-size: 0.8rem;
    color: var(--ms-text-muted);
  }
  figure {
    margin: 0;
  }
  .screen {
    aspect-ratio: 16 / 9;
    border-radius: var(--ms-radius-sm);
    overflow: hidden;
    border: 1px solid var(--ms-border);
  }
  figcaption {
    margin-top: 4px;
    font-size: 0.8rem;
    color: var(--ms-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .timers {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .timers div {
    display: grid;
    padding: 8px 12px;
    background: var(--ms-surface);
    border: 1px solid var(--ms-border);
    border-radius: var(--ms-radius-sm);
  }
  .timers span {
    font-size: 0.75rem;
    color: var(--ms-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .timers strong {
    font-family: var(--ms-font-mono);
    font-size: 1.6rem;
    font-variant-numeric: tabular-nums;
  }
  .stage .timers strong {
    font-size: 3rem;
  }
  .notes {
    margin: 0;
    font-size: 1.15rem;
    line-height: 1.45;
    white-space: pre-wrap;
  }
  .stage .notes {
    font-size: 1.5rem;
  }
  .row-panel {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .row-panel .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chips {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .countdown {
    font-family: var(--ms-font-mono);
    font-size: 1.6rem;
    font-variant-numeric: tabular-nums;
    margin-right: auto;
  }
  .countdown.over {
    color: var(--ms-danger);
  }
  .message-form {
    margin-top: 10px;
  }
  .message-form input {
    flex: 1;
    min-width: 0;
    font: inherit;
    padding: 10px;
    color: var(--ms-text);
    background: var(--ms-surface-2);
    border: 1px solid var(--ms-border);
    border-radius: var(--ms-radius-sm);
  }
  .masters {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }
  .cues {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
  }
  .cues button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 44px;
    padding: 6px 10px;
    font: inherit;
    text-align: left;
    background: var(--ms-surface-2);
    color: var(--ms-text);
    border: 2px solid transparent;
    border-radius: var(--ms-radius-sm);
  }
  .cues button.live {
    border-color: var(--ms-go);
  }
  .tag {
    width: 8px;
    height: 24px;
    border-radius: 3px;
  }
  .nav {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    display: grid;
    grid-template-columns: 1fr 2fr;
    gap: 8px;
    padding: 10px var(--ms-gap) calc(10px + env(safe-area-inset-bottom));
    background: var(--ms-surface);
    border-top: 1px solid var(--ms-border);
  }
  .nav :global(.ms-btn) {
    min-height: 110px;
  }
  .message {
    display: grid;
    gap: 12px;
    padding: 32px var(--ms-gap);
    text-align: center;
    font-size: 1.1rem;
  }
  .toast {
    position: fixed;
    left: 50%;
    bottom: 150px;
    transform: translateX(-50%);
    padding: 10px 16px;
    background: var(--ms-danger);
    color: #fff;
    border-radius: var(--ms-radius);
    font-weight: 600;
    max-width: 90vw;
  }
  @media (orientation: landscape) and (max-height: 500px) {
    .screens {
      grid-template-columns: 1fr 1fr;
    }
  }
</style>
