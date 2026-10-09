<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Lists the screens and windows the host can capture; picking one calls `onpick`. -->
<script lang="ts">
  import type { CaptureSource } from "@midnightsnack/protocol";
  import { Button, captureLabel, t, type HostConnection } from "@midnightsnack/ui";
  import { host } from "../lib/host";

  interface Props {
    conn: HostConnection;
    onpick: (source: CaptureSource) => void;
    oncancel?: () => void;
  }
  let { conn, onpick, oncancel }: Props = $props();

  const targets = $derived(conn.captureTargets);
  const isMac = navigator.userAgent.includes("Mac");

  $effect(() => {
    conn.requestCaptureTargets();
  });
</script>

<div class="picker" role="group" aria-label={t("capture.pick")}>
  <div class="head">
    <strong>{t("capture.pick")}</strong>
    <Button
      variant="ghost"
      onclick={() => conn.requestCaptureTargets()}
      aria-label={t("capture.refresh")}>⟳</Button
    >
  </div>
  {#if conn.capturePermissionMissing}
    <p class="warn" role="alert">{t("capture.permission")}</p>
    {#if isMac}
      <Button onclick={() => host.openCaptureSettings()}>{t("capture.open_settings")}</Button>
    {/if}
  {:else if targets === null}
    <p class="hint">{t("capture.loading")}</p>
  {:else if targets.length === 0}
    <p class="hint">{t("capture.none")}</p>
  {:else}
    <ul>
      {#each targets as target, i (i)}
        <li>
          <button onclick={() => onpick(target.source)}>
            <span class="kind">{t(`capture.kind_${target.source.type}`)}</span>
            <span class="label">{captureLabel(target.source) || t("cue.unnamed")}</span>
            <span class="size">{target.width}×{target.height}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
  {#if oncancel}<Button variant="ghost" onclick={oncancel}>{t("common.cancel")}</Button>{/if}
</div>

<style>
  .picker {
    display: grid;
    gap: 6px;
    padding: 8px;
    border: 1px solid var(--ms-border);
    border-radius: var(--ms-radius-sm);
    background: var(--ms-surface-2);
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 2px;
    max-height: 240px;
    overflow: auto;
  }
  li button {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 8px;
    width: 100%;
    text-align: left;
    padding: 6px 8px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  li button:hover {
    background: var(--ms-surface-3);
  }
  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .kind,
  .size,
  .hint {
    color: var(--ms-text-muted);
    font-size: 0.8rem;
  }
  .hint,
  .warn {
    margin: 0;
  }
  .warn {
    color: var(--ms-warn);
  }
</style>
