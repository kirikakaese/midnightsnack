<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!--
  Outputs of the show (feed, scaling, margin) and where each one's window goes on this host.
  The output list lives in the show; display placement is a host setting.
-->
<script lang="ts">
  import type { OutputDef, TestPattern } from "@midnightsnack/protocol";
  import { Button, Stage, t, type HostConnection } from "@midnightsnack/ui";
  import { onDestroy } from "svelte";
  import { host, onDisplaysChanged, type DisplayInfo, type OutputWindowState } from "../lib/host";
  import { isController } from "../lib/mode";

  interface Props {
    conn: HostConnection;
  }
  let { conn }: Props = $props();

  const outputs = $derived(conn.show?.outputs ?? []);
  const testPattern = $derived(conn.live?.test_pattern ?? null);

  let displays = $state<DisplayInfo[]>([]);
  let missing = $state<string[]>([]);
  let windows = $state<Record<string, OutputWindowState>>({});
  // Display picked in the form for outputs whose window is not placed yet.
  let picked = $state<Record<string, string>>({});
  let windowed = $state<Record<string, boolean>>({});
  let error = $state<string | null>(null);

  async function refresh() {
    const [status, wins] = await Promise.all([host.displayStatus(), host.outputWindows()]);
    displays = status.displays;
    missing = status.missing;
    windows = wins;
    for (const [id, w] of Object.entries(wins)) {
      windowed[id] = w.windowed;
      if (w.display && displays.some((d) => d.name === w.display)) picked[id] = w.display;
    }
  }

  // Placement is about this computer's displays; a controller only edits the show's outputs.
  $effect(() => {
    if (!isController()) refresh();
  });
  const stop = onDisplaysChanged(() => refresh());
  onDestroy(stop);

  function displayFor(id: string): string {
    if (picked[id]) return picked[id];
    const used = new Set(Object.values(windows).map((w) => w.display));
    const free = displays.find((d) => !d.primary && !used.has(d.name));
    return free?.name ?? displays.find((d) => !d.primary)?.name ?? displays[0]?.name ?? "";
  }

  async function openOutput(id: string) {
    error = null;
    try {
      await host.openOutput(id, displayFor(id) || null, windowed[id] ?? false);
    } catch (e) {
      error = String(e);
    }
    await refresh();
  }

  async function closeOutput(id: string) {
    error = null;
    try {
      await host.closeOutput(id);
    } catch (e) {
      error = String(e);
    }
    await refresh();
  }

  function put(output: OutputDef, patch: Partial<OutputDef>) {
    conn.action({ action: "put_output", output: { ...output, ...patch } });
  }

  function addOutput() {
    let n = outputs.length + 1;
    while (outputs.some((o) => o.id === `output-${n}`)) n++;
    conn.action({
      action: "put_output",
      output: {
        id: `output-${n}`,
        name: t("outputs.default_name", { n }),
        feed: "program",
        overlays: true,
        scaling: "fit",
        margin: 0,
      },
    });
  }

  async function removeOutput(id: string) {
    if (windows[id]?.window_open) await closeOutput(id);
    conn.action({ action: "remove_output", output_id: id });
  }

  function setPattern(pattern: TestPattern | null) {
    conn.action({ action: "set_test_pattern", pattern });
  }
</script>

<div class="outputs">
  <div class="head">
    <h3>{t("outputs.title")}</h3>
    {#if !isController()}
      <Button variant="ghost" onclick={refresh} aria-label={t("output.refresh")}>⟳</Button>
    {/if}
  </div>

  {#if isController()}
    <p class="hint">{t("outputs.controller_hint")}</p>
  {:else if displays.length < 2}
    <p class="warn">{t("output.single_display_hint")}</p>
  {/if}

  {#each outputs as o (o.id)}
    {@const win = windows[o.id]}
    {@const isOpen = win?.window_open ?? false}
    <section class="output" aria-label={o.name} data-output={o.id}>
      <div class="title">
        <input
          value={o.name}
          aria-label={t("outputs.name")}
          onchange={(e) => e.currentTarget.value.trim() && put(o, { name: e.currentTarget.value })}
        />
        {#if !isController()}
          <span class="state" class:on={isOpen}>
            {isOpen ? t("outputs.on_screen") : t("outputs.closed")}
          </span>
        {/if}
      </div>

      {#if missing.includes(o.id)}
        <p class="warn" role="alert">
          {t("outputs.display_missing", { display: win?.display ?? "" })}
        </p>
      {/if}

      {#if o.feed === "program"}
        <div class="preview">
          <Stage {conn} which="output" outputId={o.id} mode="thumb" />
        </div>
      {/if}

      <div class="ms-form">
        <div class="row">
          <label>
            <span>{t("outputs.feed")}</span>
            <select
              value={o.feed}
              onchange={(e) => put(o, { feed: e.currentTarget.value as OutputDef["feed"] })}
            >
              <option value="program">{t("outputs.feed_program")}</option>
              <option value="stage">{t("outputs.feed_stage")}</option>
            </select>
          </label>
          {#if o.feed === "program"}
            <label>
              <span>{t("outputs.scaling")}</span>
              <select
                value={o.scaling}
                onchange={(e) => put(o, { scaling: e.currentTarget.value as OutputDef["scaling"] })}
              >
                <option value="fit">{t("outputs.scaling_fit")}</option>
                <option value="fill">{t("outputs.scaling_fill")}</option>
                <option value="stretch">{t("outputs.scaling_stretch")}</option>
              </select>
            </label>
          {/if}
        </div>
        {#if o.feed === "program"}
          <div class="row">
            <label>
              <span>{t("outputs.margin")}</span>
              <input
                type="number"
                min="0"
                max="20"
                value={o.margin}
                onchange={(e) =>
                  put(o, {
                    margin: Math.max(
                      0,
                      Math.min(20, Math.round(Number(e.currentTarget.value) || 0)),
                    ),
                  })}
              />
            </label>
            <label class="check">
              <input
                type="checkbox"
                checked={o.overlays}
                onchange={(e) => put(o, { overlays: e.currentTarget.checked })}
              />
              {t("outputs.overlays")}
            </label>
          </div>
        {/if}

        {#if !isController()}
          <label>
            <span>{t("output.display")}</span>
            <select
              value={displayFor(o.id)}
              onchange={(e) => (picked[o.id] = e.currentTarget.value)}
            >
              {#each displays as d (d.name)}
                <option value={d.name}>
                  {d.name} — {d.width}×{d.height}{d.primary ? ` (${t("output.primary")})` : ""}
                </option>
              {/each}
            </select>
          </label>
          <label class="check">
            <input
              type="checkbox"
              checked={windowed[o.id] ?? false}
              onchange={(e) => (windowed[o.id] = e.currentTarget.checked)}
            />
            {t("output.windowed")}
          </label>
        {/if}
        <div class="row">
          {#if !isController()}
            <Button variant="go" onclick={() => openOutput(o.id)}>
              {isOpen ? t("output.move") : t("output.open")}
            </Button>
            {#if isOpen}
              <Button onclick={() => closeOutput(o.id)}>{t("output.close")}</Button>
            {/if}
          {/if}
          {#if o.id !== "main"}
            <Button variant="danger" onclick={() => removeOutput(o.id)}>
              {t("outputs.remove")}
            </Button>
          {/if}
        </div>
      </div>
    </section>
  {/each}

  <Button onclick={addOutput}>{t("outputs.add")}</Button>
  {#if error}<p class="error" role="alert">{error}</p>{/if}

  <h3>{t("outputs.test_pattern")}</h3>
  <p class="hint">{t("outputs.test_pattern_hint")}</p>
  <div class="row" role="group" aria-label={t("outputs.test_pattern")}>
    <Button
      active={testPattern === null}
      aria-pressed={testPattern === null}
      onclick={() => setPattern(null)}
    >
      {t("outputs.pattern_off")}
    </Button>
    <Button
      active={testPattern === "grid"}
      aria-pressed={testPattern === "grid"}
      onclick={() => setPattern("grid")}
    >
      {t("outputs.pattern_grid")}
    </Button>
    <Button
      active={testPattern === "bars"}
      aria-pressed={testPattern === "bars"}
      onclick={() => setPattern("bars")}
    >
      {t("outputs.pattern_bars")}
    </Button>
  </div>
</div>

<style>
  .outputs {
    display: grid;
    gap: 10px;
  }
  .head,
  .title {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .title input {
    flex: 1;
    min-width: 0;
    font-weight: 600;
  }
  h3 {
    margin: 4px 0 0;
    font-size: 0.75rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ms-text-muted);
  }
  .output {
    display: grid;
    gap: 8px;
    padding: 10px;
    border: 1px solid var(--ms-border);
    border-radius: var(--ms-radius);
  }
  .preview {
    aspect-ratio: 16 / 9;
    overflow: hidden;
    background: #000;
    border-radius: 4px;
  }
  .state {
    font-size: 0.75rem;
    color: var(--ms-text-muted);
  }
  .state.on {
    color: var(--ms-go);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: end;
  }
  .hint {
    margin: 0;
    font-size: 0.8rem;
    color: var(--ms-text-muted);
  }
  .warn {
    margin: 0;
    font-size: 0.85rem;
    color: var(--ms-warn);
  }
  .error {
    margin: 0;
    color: var(--ms-danger);
  }
</style>
