<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import { Button, Panel, t } from "@midnightsnack/ui";
  import { host, type DisplayInfo, type OutputState } from "../lib/host";

  let displays = $state<DisplayInfo[]>([]);
  let output = $state<OutputState>({ open: false, display: null, windowed: false });
  let selected = $state<string>("");
  let windowed = $state(false);
  let error = $state<string | null>(null);

  async function refresh() {
    displays = await host.listDisplays();
    output = await host.outputState();
    windowed = output.windowed;
    const remembered = displays.find((d) => d.name === output.display);
    const external = displays.find((d) => !d.primary);
    selected = remembered?.name ?? external?.name ?? displays[0]?.name ?? "";
  }

  $effect(() => {
    refresh();
  });

  async function openOutput() {
    error = null;
    try {
      await host.openOutput(selected || null, windowed);
    } catch (e) {
      error = String(e);
    }
    await refresh();
  }

  async function closeOutput() {
    await host.closeOutput();
    await refresh();
  }
</script>

<Panel title={t("output.title")}>
  {#snippet actions()}
    <Button variant="ghost" onclick={refresh} aria-label={t("output.refresh")}>⟳</Button>
  {/snippet}
  <div class="output">
    <label>
      <span>{t("output.display")}</span>
      <select bind:value={selected}>
        {#each displays as d (d.name)}
          <option value={d.name}>
            {d.name} — {d.width}×{d.height}{d.primary ? ` (${t("output.primary")})` : ""}
          </option>
        {/each}
      </select>
    </label>
    <label class="check">
      <input type="checkbox" bind:checked={windowed} />
      {t("output.windowed")}
    </label>
    {#if displays.length < 2 && !windowed}
      <p class="hint">{t("output.single_display_hint")}</p>
    {/if}
    <div class="row">
      <Button variant="go" onclick={openOutput}>
        {output.open ? t("output.move") : t("output.open")}
      </Button>
      {#if output.open}
        <Button onclick={closeOutput}>{t("output.close")}</Button>
      {/if}
    </div>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  </div>
</Panel>

<style>
  .output {
    display: grid;
    gap: 10px;
  }
  label {
    display: grid;
    gap: 4px;
    font-size: 0.85rem;
    color: var(--ms-text-muted);
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--ms-text);
  }
  select {
    font: inherit;
    padding: 8px;
    background: var(--ms-surface-2);
    color: var(--ms-text);
    border: 1px solid var(--ms-border);
    border-radius: var(--ms-radius-sm);
  }
  .row {
    display: flex;
    gap: 8px;
  }
  .hint {
    margin: 0;
    font-size: 0.85rem;
    color: var(--ms-warn);
  }
  .error {
    margin: 0;
    color: var(--ms-danger);
  }
</style>
