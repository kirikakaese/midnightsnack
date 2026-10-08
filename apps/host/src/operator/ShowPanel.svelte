<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { Transition } from "@midnightsnack/protocol";
  import { Button, t, type HostConnection } from "@midnightsnack/ui";
  import { open } from "@tauri-apps/plugin-dialog";
  import ThemeEditor from "./ThemeEditor.svelte";

  interface Props {
    conn: HostConnection;
  }
  let { conn }: Props = $props();
  const show = $derived(conn.show);
  const imageFilter = [
    { name: t("cue.filter_images"), extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp"] },
  ];

  function setTransition(patch: Partial<Transition>) {
    if (!show) return;
    conn.action({
      action: "set_default_transition",
      transition: { ...show.default_transition, ...patch },
    });
  }

  async function pick(): Promise<string | null> {
    const path = await open({ multiple: false, filters: imageFilter });
    return typeof path === "string" ? path : null;
  }
</script>

{#if show}
  <div class="ms-form">
    <h3>{t("show_settings.transition")}</h3>
    <div class="row">
      <select
        value={show.default_transition.kind}
        aria-label={t("show_settings.transition")}
        onchange={(e) =>
          setTransition({
            kind: e.currentTarget.value === "fade" ? "fade" : "cut",
            duration_ms: show.default_transition.duration_ms || 400,
          })}
      >
        <option value="cut">{t("transition.cut")}</option>
        <option value="fade">{t("transition.fade")}</option>
      </select>
      {#if show.default_transition.kind === "fade"}
        <label>
          <span>{t("inspector.duration_ms")}</span>
          <input
            type="number"
            min="0"
            max="10000"
            step="50"
            value={show.default_transition.duration_ms}
            onchange={(e) =>
              setTransition({
                duration_ms: Math.max(0, Math.min(10000, Number(e.currentTarget.value) || 0)),
              })}
          />
        </label>
      {/if}
    </div>
    <p class="hint">{t("show_settings.transition_hint")}</p>

    <h3>{t("show_settings.theme")}</h3>
    <ThemeEditor
      theme={show.default_theme}
      onchange={(theme) => conn.action({ action: "set_default_theme", theme })}
      onpickimage={async () => {
        const path = await pick();
        if (path) conn.action({ action: "set_background_image", cue_id: null, path });
      }}
      onclearimage={() => conn.action({ action: "set_background_image", cue_id: null, path: null })}
    />

    <h3>{t("show_settings.logo")}</h3>
    <div class="row">
      <Button
        onclick={async () => {
          const path = await pick();
          if (path) conn.action({ action: "set_logo_image", path });
        }}>{t("show_settings.choose_logo")}</Button
      >
      {#if show.logo}
        <Button
          variant="ghost"
          onclick={() => conn.action({ action: "set_logo_image", path: null })}
        >
          {t("show_settings.builtin_logo")}
        </Button>
      {/if}
    </div>
  </div>
{/if}
