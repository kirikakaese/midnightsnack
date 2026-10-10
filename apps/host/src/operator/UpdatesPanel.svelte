<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Control tab → Updates: automatic checks, how often, automatic installation on quit, betas. -->
<script lang="ts">
  import { Button, getLocale, t, type MessageKey } from "@midnightsnack/ui";
  import { updates, type UpdateInterval, type UpdateSettings } from "../lib/updates.svelte";

  const INTERVALS: Record<UpdateInterval, MessageKey> = {
    daily: "updates.interval.daily",
    weekly: "updates.interval.weekly",
    monthly: "updates.interval.monthly",
  };

  const info = $derived(updates.info);
  let busy = $state(false);
  let installError = $state<string | null>(null);

  function save(patch: Partial<UpdateSettings>) {
    if (!info) return;
    updates.setSettings({ ...info.settings, ...patch });
  }

  async function checkNow() {
    busy = true;
    await updates.check().catch(() => {});
    busy = false;
  }

  async function install() {
    installError = null;
    busy = true;
    try {
      await updates.install();
    } catch (e) {
      installError = String(e);
    }
    busy = false;
  }

  const lastChecked = $derived.by(() => {
    const ms = info?.settings.last_check_ms;
    if (ms == null) return t("updates.never_checked");
    const when = new Intl.DateTimeFormat(getLocale(), {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(new Date(ms));
    return t("updates.last_checked", { when });
  });

  const statusText = $derived.by(() => {
    const s = info?.status;
    switch (s?.state) {
      case "checking":
        return t("updates.status.checking");
      case "up_to_date":
        return t("updates.status.up_to_date");
      case "available":
        return t("updates.status.available", { version: s.version });
      case "downloading":
        return t("updates.status.downloading", { version: s.version, percent: s.percent ?? 0 });
      case "ready":
        return t("updates.status.ready", { version: s.version });
      case "installing":
        return t("updates.status.installing", { version: s.version });
      case "failed":
        return t("updates.status.failed", { message: s.message });
      default:
        return "";
    }
  });
  const canInstall = $derived(info?.status.state === "available" || info?.status.state === "ready");
</script>

<section aria-label={t("updates.title")}>
  <h3>{t("updates.title")}</h3>
  {#if info}
    <p class="muted">{t("updates.version", { version: info.current_version })}</p>
    {#if info.supported}
      <label class="check">
        <input
          type="checkbox"
          checked={info.settings.automatically_check}
          onchange={(e) => save({ automatically_check: e.currentTarget.checked })}
        />
        {t("updates.auto_check")}
      </label>
      <label class="row">
        <span>{t("updates.interval")}</span>
        <select
          value={info.settings.interval}
          disabled={!info.settings.automatically_check}
          onchange={(e) => save({ interval: e.currentTarget.value as UpdateInterval })}
        >
          {#each Object.entries(INTERVALS) as [value, key] (value)}
            <option {value}>{t(key)}</option>
          {/each}
        </select>
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={info.settings.automatically_install}
          disabled={!info.settings.automatically_check}
          onchange={(e) => save({ automatically_install: e.currentTarget.checked })}
        />
        {t("updates.auto_install")}
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={info.settings.include_beta}
          onchange={(e) => save({ include_beta: e.currentTarget.checked })}
        />
        {t("updates.include_beta")}
      </label>
      <p class="muted">{lastChecked}</p>
      <div class="row">
        <Button disabled={busy || info.status.state === "checking"} onclick={checkNow}>
          {t("updates.check_now")}
        </Button>
        {#if canInstall}
          <Button variant="go" disabled={busy} onclick={install}>{t("updates.install")}</Button>
        {/if}
      </div>
      {#if statusText}<p role="status">{statusText}</p>{/if}
      {#if installError === "outputs_open"}
        <p class="warn" role="alert">{t("updates.outputs_open")}</p>
      {:else if installError}
        <p class="warn" role="alert">{t("updates.status.failed", { message: installError })}</p>
      {/if}
      <p class="muted">{t("updates.hint")}</p>
    {:else if info.package_managed}
      <p class="muted">{t("updates.package_managed")}</p>
    {:else}
      <p class="muted">{t("updates.unsupported")}</p>
    {/if}
  {/if}
</section>

<style>
  section {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 8px;
  }
  h3 {
    margin: 4px 0 0;
    font-size: 0.75rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ms-text-muted);
  }
  p {
    margin: 0;
    font-size: 0.85rem;
  }
  .muted {
    color: var(--ms-text-muted);
    font-size: 0.8rem;
  }
  .warn {
    color: var(--ms-warn);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .check {
    display: flex;
    gap: 8px;
    align-items: center;
  }
</style>
