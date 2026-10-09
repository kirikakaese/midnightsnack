<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Open/move/close one display window (audience output or stage display). -->
<script lang="ts">
  import { Button, t } from "@midnightsnack/ui";
  import { host, type DisplayInfo, type WindowKind, type WindowState } from "../lib/host";

  interface Props {
    kind: WindowKind;
    displays: DisplayInfo[];
  }
  let { kind, displays }: Props = $props();

  let win = $state<WindowState>({ open: false, display: null, windowed: false });
  let selected = $state("");
  let windowed = $state(false);
  let error = $state<string | null>(null);

  async function refresh() {
    win = await host.windowState(kind);
    windowed = win.windowed;
    const remembered = displays.find((d) => d.name === win.display);
    const external = displays.find((d) => !d.primary);
    selected = remembered?.name ?? external?.name ?? displays[0]?.name ?? "";
  }

  $effect(() => {
    void displays;
    refresh();
  });

  async function openWindow() {
    error = null;
    try {
      await host.openWindow(kind, selected || null, windowed);
    } catch (e) {
      error = String(e);
    }
    await refresh();
  }
</script>

<div class="ms-form">
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
  {#if kind === "output" && displays.length < 2 && !windowed}
    <p class="warn">{t("output.single_display_hint")}</p>
  {/if}
  <div class="row">
    <Button variant="go" onclick={openWindow}
      >{win.open ? t("output.move") : t(`window.open_${kind}`)}</Button
    >
    {#if win.open}
      <Button
        onclick={async () => {
          await host.closeWindow(kind);
          await refresh();
        }}>{t("output.close")}</Button
      >
    {/if}
  </div>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</div>

<style>
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
