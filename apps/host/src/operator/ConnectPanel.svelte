<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { Role } from "@midnightsnack/protocol";
  import { Button, t, type HostConnection } from "@midnightsnack/ui";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { host } from "../lib/host";

  interface Props {
    conn: HostConnection;
  }
  let { conn }: Props = $props();

  const ROLES: Role[] = ["admin", "operator", "presenter", "stage_viewer"];
  const joinUrl = $derived(conn.pairing?.join_urls[0] ?? null);
  let qr = $state<string | null>(null);
  let approveRole = $state<Record<string, Role>>({});
  const remotes = $derived(conn.devices.filter((d) => !d.local));

  $effect(() => {
    const url = joinUrl;
    if (!url) return;
    host.qrSvg(url).then((svg) => {
      // Generated locally from a URL we built; render as an image to avoid injecting markup.
      qr = `data:image/svg+xml;base64,${btoa(svg)}`;
    });
  });

  function approve(id: string) {
    conn.action({
      action: "approve_pairing",
      request_id: id,
      role: approveRole[id] ?? "presenter",
    });
  }

  async function disconnectAll() {
    const ok = await ask(t("devices.disconnect_all_question"), {
      title: t("devices.disconnect_all"),
      kind: "warning",
    });
    if (ok) conn.action({ action: "disconnect_all" });
  }

  function setAutoApprove(value: string) {
    conn.action({ action: "set_auto_approve", role: value === "" ? null : (value as Role) });
  }
</script>

<div class="connect-wrap">
  <div class="connect">
    {#if conn.pairing}
      <div class="pair">
        {#if qr}<img class="qr" src={qr} alt={t("connect.qr_alt")} />{/if}
        <div class="pin">
          <span class="label">{t("connect.pin")}</span>
          <span class="digits" aria-label={t("connect.pin")}>{conn.pairing.pin}</span>
        </div>
      </div>
      <details>
        <summary>{t("connect.addresses")}</summary>
        <ul class="urls">
          {#each conn.pairing.join_urls as url (url)}
            <li><code>{url.split("/join")[0]}</code></li>
          {/each}
        </ul>
      </details>
      <label class="auto">
        <span>{t("connect.auto_approve")}</span>
        <select
          value={conn.pairing.auto_approve ?? ""}
          onchange={(e) => setAutoApprove((e.currentTarget as HTMLSelectElement).value)}
        >
          <option value="">{t("connect.auto_approve_off")}</option>
          {#each ROLES.filter((r) => r !== "admin") as r (r)}
            <option value={r}>{t(`role.${r}`)}</option>
          {/each}
        </select>
      </label>
      <label class="check">
        <input
          type="checkbox"
          checked={conn.autoAcceptUploads}
          onchange={(e) =>
            conn.action({ action: "set_auto_accept_uploads", on: e.currentTarget.checked })}
        />
        {t("inbox.auto_accept")}
      </label>
    {/if}

    {#if conn.pending.length > 0}
      <h3>{t("devices.pending")}</h3>
      <ul class="list">
        {#each conn.pending as p (p.request_id)}
          <li class="pending" role="alert">
            <div class="who">
              <strong>{p.device_name}</strong>
              <span class="muted">{p.address}</span>
            </div>
            <select bind:value={approveRole[p.request_id]} aria-label={t("devices.role")}>
              {#each ROLES as r (r)}
                <option value={r} selected={r === "presenter"}>{t(`role.${r}`)}</option>
              {/each}
            </select>
            <Button variant="go" onclick={() => approve(p.request_id)}
              >{t("devices.approve")}</Button
            >
            <Button
              variant="danger"
              onclick={() => conn.action({ action: "deny_pairing", request_id: p.request_id })}
            >
              {t("devices.deny")}
            </Button>
          </li>
        {/each}
      </ul>
    {/if}

    <h3>{t("devices.title")}</h3>
    {#if remotes.length === 0}
      <p class="muted">{t("devices.none")}</p>
    {:else}
      <ul class="list">
        {#each remotes as d (d.id)}
          <li>
            <span class="dot" class:on={d.connected} aria-hidden="true"></span>
            <div class="who">
              <strong>{d.name}</strong>
              <span class="muted">
                {d.connected
                  ? d.latency_ms != null
                    ? t("devices.latency", { ms: d.latency_ms })
                    : t("status.connected")
                  : t("status.disconnected")}
              </span>
            </div>
            <select
              value={d.role}
              aria-label={t("devices.role")}
              onchange={(e) =>
                conn.action({
                  action: "set_device_role",
                  device_id: d.id,
                  role: (e.currentTarget as HTMLSelectElement).value as Role,
                })}
            >
              {#each ROLES as r (r)}
                <option value={r}>{t(`role.${r}`)}</option>
              {/each}
            </select>
            <Button
              variant="ghost"
              aria-label={t("devices.revoke")}
              onclick={() => conn.action({ action: "revoke_device", device_id: d.id })}>✕</Button
            >
          </li>
        {/each}
      </ul>
      <Button variant="danger" onclick={disconnectAll}>{t("devices.disconnect_all")}</Button>
    {/if}
  </div>
</div>

<style>
  .connect {
    display: grid;
    gap: 10px;
  }
  .pair {
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .qr {
    width: 140px;
    height: 140px;
    border-radius: 6px;
    background: #fff;
  }
  .pin {
    display: grid;
  }
  .label {
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ms-text-muted);
  }
  .digits {
    font-family: var(--ms-font-mono);
    font-size: 2rem;
    letter-spacing: 0.15em;
  }
  .urls {
    margin: 6px 0 0;
    padding-left: 18px;
  }
  h3 {
    margin: 6px 0 0;
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ms-text-muted);
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  .list li {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .pending {
    padding: 6px;
    border: 2px solid var(--ms-warn);
    border-radius: var(--ms-radius-sm);
    flex-wrap: wrap;
  }
  .who {
    display: grid;
    flex: 1;
    min-width: 0;
  }
  .muted {
    color: var(--ms-text-muted);
    font-size: 0.85rem;
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--ms-text-muted);
  }
  .dot.on {
    background: var(--ms-go);
  }
  select {
    font: inherit;
    padding: 6px;
    background: var(--ms-surface-2);
    color: var(--ms-text);
    border: 1px solid var(--ms-border);
    border-radius: var(--ms-radius-sm);
  }
  .auto {
    display: grid;
    gap: 4px;
    font-size: 0.85rem;
    color: var(--ms-text-muted);
  }
</style>
