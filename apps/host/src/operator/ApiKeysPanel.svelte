<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- API keys for control surfaces (Companion, scripts, OSC): create, restrict, revoke. -->
<script lang="ts">
  import type { Role } from "@midnightsnack/protocol";
  import { Button, t, type HostConnection } from "@midnightsnack/ui";

  interface Props {
    conn: HostConnection;
  }
  let { conn }: Props = $props();

  const ROLES: Role[] = ["operator", "presenter", "admin", "stage_viewer"];
  const keys = $derived(conn.devices.filter((d) => d.api_key));
  let name = $state("");
  let role = $state<Role>("operator");
  let copied = $state(false);

  function create(e: SubmitEvent) {
    e.preventDefault();
    if (!name.trim()) return;
    copied = false;
    conn.createApiKey(name.trim(), role);
    name = "";
  }

  async function copy(token: string) {
    try {
      await navigator.clipboard.writeText(token);
      copied = true;
    } catch {
      copied = false;
    }
  }
</script>

<section class="keys" aria-label={t("apikeys.title")}>
  <h3>{t("apikeys.title")}</h3>
  <p class="muted">{t("apikeys.hint")}</p>

  {#if keys.length}
    <ul class="list">
      {#each keys as k (k.id)}
        <li>
          <span class="dot" class:on={k.connected} aria-hidden="true"></span>
          <div class="who">
            <strong>{k.name}</strong>
            <span class="muted">{t(`role.${k.role}`)}</span>
          </div>
          <Button
            variant="ghost"
            aria-label={t("apikeys.revoke", { name: k.name })}
            onclick={() => conn.action({ action: "revoke_device", device_id: k.id })}>✕</Button
          >
        </li>
      {/each}
    </ul>
  {/if}

  {#if conn.newApiKey}
    <div class="token" role="alert">
      <span>{t("apikeys.created", { name: conn.newApiKey.name })}</span>
      <code>{conn.newApiKey.token}</code>
      <div class="row">
        <Button onclick={() => copy(conn.newApiKey!.token)}>
          {copied ? t("apikeys.copied") : t("apikeys.copy")}
        </Button>
        <Button variant="ghost" onclick={() => (conn.newApiKey = null)}>{t("apikeys.done")}</Button>
      </div>
    </div>
  {/if}

  <form class="row" onsubmit={create}>
    <input bind:value={name} placeholder={t("apikeys.name")} aria-label={t("apikeys.name")} />
    <select bind:value={role} aria-label={t("devices.role")}>
      {#each ROLES as r (r)}
        <option value={r}>{t(`role.${r}`)}</option>
      {/each}
    </select>
    <Button type="submit" disabled={!name.trim()}>{t("apikeys.create")}</Button>
  </form>

  <label class="check">
    <input
      type="checkbox"
      checked={conn.apiLocalOnly}
      onchange={(e) => conn.action({ action: "set_api_local_only", on: e.currentTarget.checked })}
    />
    {t("apikeys.local_only")}
  </label>
</section>

<style>
  .keys {
    display: grid;
    gap: 8px;
  }
  h3 {
    margin: 6px 0 0;
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ms-text-muted);
  }
  .muted {
    margin: 0;
    font-size: 0.8rem;
    color: var(--ms-text-muted);
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .who {
    display: grid;
    flex: 1;
    min-width: 0;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ms-text-muted);
  }
  .dot.on {
    background: var(--ms-go);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .row input {
    flex: 1;
    min-width: 120px;
  }
  .token {
    display: grid;
    gap: 6px;
    padding: 8px;
    border: 1px solid var(--ms-warn);
    border-radius: var(--ms-radius-sm);
  }
  code {
    font-family: var(--ms-font-mono);
    font-size: 0.8rem;
    word-break: break-all;
    user-select: all;
  }
  .check {
    display: flex;
    gap: 6px;
    align-items: center;
  }
</style>
