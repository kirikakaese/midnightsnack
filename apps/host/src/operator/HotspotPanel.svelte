<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- This laptop as its own Wi-Fi: started from here with NetworkManager, otherwise guided
     through the system settings. A QR code lets phones join the Wi-Fi in one step. -->
<script lang="ts">
  import { Button, t } from "@midnightsnack/ui";
  import { host, type HotspotStatus } from "../lib/host";
  import { wifiQrText, randomPassword } from "../lib/wifi";

  const SSID_KEY = "midnightsnack.hotspot.ssid";
  const PASSWORD_KEY = "midnightsnack.hotspot.password";

  function stored(key: string, fallback: () => string): string {
    try {
      const v = localStorage.getItem(key);
      if (v) return v;
      const f = fallback();
      localStorage.setItem(key, f);
      return f;
    } catch {
      return fallback();
    }
  }
  function remember(key: string, value: string) {
    try {
      localStorage.setItem(key, value);
    } catch {
      // Not remembered; fine.
    }
  }

  let status = $state<HotspotStatus | null>(null);
  let ssid = $state(stored(SSID_KEY, () => "DECK"));
  let password = $state(stored(PASSWORD_KEY, randomPassword));
  let busy = $state(false);
  let error = $state<string | null>(null);
  let wifiQr = $state<string | null>(null);
  let showQr = $state(false);

  async function refresh() {
    status = await host.hotspotStatus();
    if (status.active && status.ssid) ssid = status.ssid;
  }
  $effect(() => {
    refresh();
  });

  $effect(() => {
    const text = wifiQrText(ssid, password);
    if (!showQr || !text) {
      wifiQr = null;
      return;
    }
    host.qrSvg(text).then((svg) => (wifiQr = `data:image/svg+xml;base64,${btoa(svg)}`));
  });

  function errorText(e: unknown): string {
    const code = String(e);
    if (code === "invalid_ssid" || code === "invalid_password" || code === "unsupported") {
      return t(`hotspot.error.${code}`);
    }
    return t("hotspot.error.failed", { message: code });
  }

  async function start() {
    error = null;
    busy = true;
    remember(SSID_KEY, ssid);
    remember(PASSWORD_KEY, password);
    try {
      status = await host.hotspotStart(ssid, password);
      showQr = true;
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  async function stop() {
    error = null;
    busy = true;
    try {
      status = await host.hotspotStop();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  const guide = $derived(
    status?.os === "windows" ? "windows" : status?.os === "macos" ? "macos" : "linux",
  );
</script>

{#if status}
  <section class="ms-form" aria-label={t("hotspot.title")}>
    <h3>{t("hotspot.title")}</h3>
    <p class="hint">{t("hotspot.hint")}</p>
    {#if status.can_start}
      {#if status.active}
        <p class="ok">{t("hotspot.active", { ssid: status.ssid ?? ssid })}</p>
      {/if}
      <div class="row">
        <label>
          <span>{t("hotspot.ssid")}</span>
          <input bind:value={ssid} maxlength="32" disabled={status.active} />
        </label>
        <label>
          <span>{t("hotspot.password")}</span>
          <input bind:value={password} maxlength="63" disabled={status.active} />
        </label>
      </div>
      <div class="row">
        {#if status.active}
          <Button disabled={busy} onclick={stop}>{t("hotspot.stop")}</Button>
        {:else}
          <Button variant="go" disabled={busy} onclick={start}>{t("hotspot.start")}</Button>
        {/if}
        <Button variant="ghost" onclick={() => (showQr = !showQr)}>
          {showQr ? t("hotspot.hide_qr") : t("hotspot.show_qr")}
        </Button>
      </div>
    {:else}
      <ol class="steps">
        {#each [1, 2, 3, 4] as const as n (n)}
          <li>{t(`hotspot.guide.${guide}.${n}`)}</li>
        {/each}
      </ol>
      <div class="row">
        <Button onclick={() => host.openHotspotSettings()}>{t("hotspot.open_settings")}</Button>
      </div>
      <p class="hint">{t("hotspot.qr_hint")}</p>
      <div class="row">
        <label>
          <span>{t("hotspot.ssid")}</span>
          <input bind:value={ssid} maxlength="32" />
        </label>
        <label>
          <span>{t("hotspot.password")}</span>
          <input bind:value={password} maxlength="63" />
        </label>
      </div>
      <div class="row">
        <Button
          variant="ghost"
          onclick={() => {
            remember(SSID_KEY, ssid);
            remember(PASSWORD_KEY, password);
            showQr = !showQr;
          }}
        >
          {showQr ? t("hotspot.hide_qr") : t("hotspot.show_qr")}
        </Button>
      </div>
    {/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    {#if showQr && wifiQr}
      <figure>
        <img class="qr" src={wifiQr} alt={t("hotspot.qr_alt", { ssid })} />
        <figcaption class="hint">{t("hotspot.qr_caption")}</figcaption>
      </figure>
    {/if}
  </section>
{/if}

<style>
  .row label {
    flex: 1 1 10em;
  }
  .steps {
    margin: 0;
    padding-left: 20px;
    display: grid;
    gap: 4px;
    font-size: 0.85rem;
  }
  .ok {
    margin: 0;
    color: var(--ms-go);
  }
  .error {
    margin: 0;
    color: var(--ms-danger);
  }
  figure {
    margin: 0;
    display: grid;
    gap: 6px;
    justify-items: start;
  }
  .qr {
    width: 160px;
    height: 160px;
    border-radius: 6px;
    background: #fff;
  }
</style>
