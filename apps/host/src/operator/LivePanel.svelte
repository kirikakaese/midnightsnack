<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { Overlay, OverlayKind, OverlayPosition } from "@midnightsnack/protocol";
  import {
    Button,
    elapsedMs,
    formatDuration,
    parseDuration,
    t,
    type HostConnection,
    type Ticker,
  } from "@midnightsnack/ui";
  import { open } from "@tauri-apps/plugin-dialog";

  interface Props {
    conn: HostConnection;
    ticker: Ticker;
  }
  let { conn, ticker }: Props = $props();

  const countdown = $derived(conn.live?.countdown ?? null);
  const left = $derived(
    countdown
      ? countdown.duration_ms - elapsedMs(countdown.elapsed, ticker.now + conn.clockOffset)
      : 0,
  );
  const running = $derived(countdown?.elapsed.running_since_ms != null);
  let durationText = $derived(countdown ? formatDuration(countdown.duration_ms) : "5:00");
  let label = $derived(countdown?.label ?? "");
  let message = $state("");
  let editing = $state<string | null>(null);

  const overlays = $derived(conn.show?.overlays ?? []);
  const visible = $derived(conn.live?.overlays_visible ?? []);
  const POSITIONS: OverlayPosition[] = [
    "top_left",
    "top_center",
    "top_right",
    "bottom_left",
    "bottom_center",
    "bottom_right",
  ];
  const KINDS = ["lower_third", "logo_bug", "clock", "ticker", "countdown"] as const;

  function setCountdown() {
    const ms = parseDuration(durationText);
    if (ms) conn.action({ action: "countdown_set", duration_ms: ms, label });
  }

  function newOverlay(type: (typeof KINDS)[number]) {
    const kinds: Record<string, OverlayKind> = {
      lower_third: { type: "lower_third", title: t("overlay.default_title"), subtitle: "" },
      logo_bug: { type: "logo_bug", image: null },
      clock: { type: "clock", seconds: false },
      ticker: { type: "ticker", text: t("overlay.default_ticker"), speed: 10 },
      countdown: { type: "countdown" },
    };
    const kind = kinds[type]!;
    const overlay: Overlay = {
      id: crypto.randomUUID(),
      name: t(`overlay.kind.${type}`),
      kind,
      position: type === "logo_bug" || type === "clock" ? "top_right" : "bottom_left",
      color: "#ffffff",
      background: type === "logo_bug" ? "#000000" : "#1c2230",
      scale: 100,
    };
    conn.action({ action: "put_overlay", overlay });
    editing = overlay.id;
  }

  function update(o: Overlay, patch: Partial<Overlay>) {
    conn.action({ action: "put_overlay", overlay: { ...o, ...patch } });
  }

  function updateKind(o: Overlay, patch: Record<string, unknown>) {
    update(o, { kind: { ...o.kind, ...patch } as OverlayKind });
  }

  async function pickBug(o: Overlay) {
    const path = await open({
      multiple: false,
      filters: [
        { name: t("cue.filter_images"), extensions: ["png", "jpg", "jpeg", "webp", "gif"] },
      ],
    });
    if (typeof path === "string")
      conn.action({ action: "set_overlay_image", overlay_id: o.id, path });
  }
</script>

<div class="ms-form">
  <h3>{t("countdown.title")}</h3>
  <div class="countdown" class:over={left < 0} aria-live="off">
    {left < 0 ? "+" : ""}{formatDuration(Math.abs(left) + (left > 0 ? 999 : 0))}
  </div>
  <div class="row">
    <input
      bind:value={durationText}
      size="7"
      aria-label={t("inspector.duration")}
      onchange={setCountdown}
    />
    <input
      bind:value={label}
      placeholder={t("inspector.label")}
      aria-label={t("inspector.label")}
      onchange={setCountdown}
    />
  </div>
  <div class="row">
    {#if running}
      <Button onclick={() => conn.action({ action: "countdown_pause" })}>{t("timer.pause")}</Button>
    {:else}
      <Button variant="go" onclick={() => conn.action({ action: "countdown_start" })}
        >{t("timer.start")}</Button
      >
    {/if}
    <Button onclick={() => conn.action({ action: "countdown_reset" })}>{t("timer.reset")}</Button>
  </div>

  <h3>{t("stage_message.title")}</h3>
  {#if conn.live?.stage_message}
    <p class="current-message">{conn.live.stage_message}</p>
  {/if}
  <form
    class="row"
    onsubmit={(e) => {
      e.preventDefault();
      conn.action({ action: "set_stage_message", text: message });
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
    {#if conn.live?.stage_message}
      <Button
        variant="ghost"
        onclick={() => conn.action({ action: "set_stage_message", text: null })}
      >
        {t("stage_message.clear")}
      </Button>
    {/if}
  </form>

  <h3>{t("overlay.title")}</h3>
  <ul class="overlays">
    {#each overlays as o (o.id)}
      <li>
        <div class="row">
          <Button
            variant="go"
            active={visible.includes(o.id)}
            onclick={() => conn.action({ action: "toggle_overlay", overlay_id: o.id })}
          >
            {o.name || t(`overlay.kind.${o.kind.type}`)}
          </Button>
          <Button
            variant="ghost"
            aria-label={t("overlay.edit")}
            onclick={() => (editing = editing === o.id ? null : o.id)}>✎</Button
          >
          <Button
            variant="ghost"
            aria-label={t("overlay.remove")}
            onclick={() => conn.action({ action: "remove_overlay", overlay_id: o.id })}>✕</Button
          >
        </div>
        {#if editing === o.id}
          <div class="editor">
            <label>
              <span>{t("inspector.name")}</span>
              <input value={o.name} onchange={(e) => update(o, { name: e.currentTarget.value })} />
            </label>
            {#if o.kind.type === "lower_third"}
              <label>
                <span>{t("overlay.title_line")}</span>
                <input
                  value={o.kind.title}
                  onchange={(e) => updateKind(o, { title: e.currentTarget.value })}
                />
              </label>
              <label>
                <span>{t("overlay.subtitle_line")}</span>
                <input
                  value={o.kind.subtitle}
                  onchange={(e) => updateKind(o, { subtitle: e.currentTarget.value })}
                />
              </label>
            {:else if o.kind.type === "ticker"}
              <label>
                <span>{t("overlay.ticker_text")}</span>
                <textarea
                  rows="2"
                  value={o.kind.text}
                  onchange={(e) => updateKind(o, { text: e.currentTarget.value })}
                ></textarea>
              </label>
              <label>
                <span>{t("overlay.speed")}</span>
                <input
                  type="range"
                  min="2"
                  max="40"
                  value={o.kind.speed}
                  onchange={(e) => updateKind(o, { speed: Number(e.currentTarget.value) })}
                />
              </label>
            {:else if o.kind.type === "clock"}
              <label class="check">
                <input
                  type="checkbox"
                  checked={o.kind.seconds}
                  onchange={(e) => updateKind(o, { seconds: e.currentTarget.checked })}
                />
                {t("overlay.seconds")}
              </label>
            {:else if o.kind.type === "logo_bug"}
              <div class="row">
                <Button onclick={() => pickBug(o)}>{t("overlay.choose_image")}</Button>
                {#if o.kind.image}
                  <Button
                    variant="ghost"
                    onclick={() =>
                      conn.action({ action: "set_overlay_image", overlay_id: o.id, path: null })}
                  >
                    {t("theme.clear_image")}
                  </Button>
                {/if}
              </div>
            {/if}
            {#if o.kind.type !== "ticker"}
              <label>
                <span>{t("overlay.position")}</span>
                <select
                  value={o.position}
                  onchange={(e) =>
                    update(o, { position: e.currentTarget.value as OverlayPosition })}
                >
                  {#each POSITIONS as p (p)}<option value={p}>{t(`position.${p}`)}</option>{/each}
                </select>
              </label>
            {:else}
              <label>
                <span>{t("overlay.position")}</span>
                <select
                  value={o.position.startsWith("top") ? "top_center" : "bottom_center"}
                  onchange={(e) =>
                    update(o, { position: e.currentTarget.value as OverlayPosition })}
                >
                  <option value="top_center">{t("position.top")}</option>
                  <option value="bottom_center">{t("position.bottom")}</option>
                </select>
              </label>
            {/if}
            <div class="row">
              <label class="check"
                ><input
                  type="color"
                  value={o.color}
                  onchange={(e) => update(o, { color: e.currentTarget.value })}
                />{t("theme.text_color")}</label
              >
              <label class="check"
                ><input
                  type="color"
                  value={o.background}
                  onchange={(e) => update(o, { background: e.currentTarget.value })}
                />{t("theme.background")}</label
              >
            </div>
            <label>
              <span>{t("overlay.scale", { n: o.scale })}</span>
              <input
                type="range"
                min="50"
                max="250"
                step="10"
                value={o.scale}
                onchange={(e) => update(o, { scale: Number(e.currentTarget.value) })}
              />
            </label>
          </div>
        {/if}
      </li>
    {/each}
  </ul>
  <label>
    <span>{t("overlay.add")}</span>
    <select
      value=""
      onchange={(e) => {
        const v = e.currentTarget.value;
        e.currentTarget.value = "";
        if (v) newOverlay(v as (typeof KINDS)[number]);
      }}
    >
      <option value="" disabled>{t("overlay.add_choose")}</option>
      {#each KINDS as k (k)}<option value={k}>{t(`overlay.kind.${k}`)}</option>{/each}
    </select>
  </label>
</div>

<style>
  .countdown {
    font-family: var(--ms-font-mono);
    font-size: 2.4rem;
    font-variant-numeric: tabular-nums;
  }
  .countdown.over {
    color: var(--ms-danger);
  }
  .current-message {
    margin: 0;
    padding: 8px 10px;
    border-radius: var(--ms-radius-sm);
    background: var(--ms-warn);
    color: #241500;
    font-weight: 600;
  }
  .overlays {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 8px;
  }
  .editor {
    display: grid;
    gap: 8px;
    margin: 8px 0 4px;
    padding: 10px;
    border: 1px solid var(--ms-border);
    border-radius: var(--ms-radius-sm);
  }
  form.row input {
    flex: 1;
  }
</style>
