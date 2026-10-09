<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { TextAlign, TextTheme } from "@midnightsnack/protocol";
  import { Button, t } from "@midnightsnack/ui";

  interface Props {
    theme: TextTheme;
    onchange: (theme: TextTheme) => void;
    /** Omitted where the host's files are not reachable (controller mode). */
    onpickimage?: () => void;
    onclearimage: () => void;
  }
  let { theme, onchange, onpickimage, onclearimage }: Props = $props();

  const FONTS = [
    "Inter, system-ui, sans-serif",
    "Georgia, 'Times New Roman', serif",
    "'Helvetica Neue', Arial, sans-serif",
    "'Trebuchet MS', sans-serif",
    "'Courier New', monospace",
  ];

  function set<K extends keyof TextTheme>(key: K, value: TextTheme[K]) {
    onchange({ ...theme, [key]: value });
  }
</script>

<div class="theme">
  <label>
    <span>{t("theme.font")}</span>
    <select value={theme.font_family} onchange={(e) => set("font_family", e.currentTarget.value)}>
      {#each FONTS.includes(theme.font_family) ? FONTS : [theme.font_family, ...FONTS] as f (f)}
        <option value={f}>{f.split(",")[0]?.replace(/'/g, "")}</option>
      {/each}
    </select>
  </label>
  <div class="row">
    <label class="check">
      <input
        type="checkbox"
        checked={theme.font_size === null}
        onchange={(e) => set("font_size", e.currentTarget.checked ? null : 10)}
      />
      {t("theme.auto_fit")}
    </label>
    {#if theme.font_size !== null}
      <label>
        <span>{t("theme.size")}</span>
        <input
          type="number"
          min="1"
          max="100"
          value={theme.font_size}
          onchange={(e) =>
            set("font_size", Math.max(1, Math.min(100, Number(e.currentTarget.value) || 10)))}
        />
      </label>
    {/if}
  </div>
  <div class="row">
    <label class="color">
      <input
        type="color"
        value={theme.color}
        onchange={(e) => set("color", e.currentTarget.value)}
      />
      {t("theme.text_color")}
    </label>
    <label class="color">
      <input
        type="color"
        value={theme.background}
        onchange={(e) => set("background", e.currentTarget.value)}
      />
      {t("theme.background")}
    </label>
  </div>
  <div class="row" role="radiogroup" aria-label={t("theme.align")}>
    {#each ["left", "center", "right"] as const as a (a)}
      <label class="check">
        <input
          type="radio"
          checked={theme.align === a}
          onchange={() => set("align", a as TextAlign)}
        />
        {t(`theme.align_${a}`)}
      </label>
    {/each}
  </div>
  <div class="row">
    {#if onpickimage}
      <Button onclick={onpickimage}>{t("theme.background_image")}</Button>
    {/if}
    {#if theme.background_image}
      <Button variant="ghost" onclick={onclearimage}>{t("theme.clear_image")}</Button>
    {/if}
  </div>
</div>

<style>
  .theme {
    display: grid;
    gap: 10px;
  }
  .color {
    display: flex !important;
    align-items: center;
    gap: 8px;
    color: var(--ms-text) !important;
  }
</style>
