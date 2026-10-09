<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Pairing (QR code and PIN), waiting requests, paired devices and how remotes connect. -->
<script lang="ts">
  import type { DeviceInfo, JoinLink, Role } from "@midnightsnack/protocol";
  import { Button, t, type HostConnection } from "@midnightsnack/ui";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { host } from "../lib/host";
  import { isController } from "../lib/mode";
  import ConnectivityPanel from "./ConnectivityPanel.svelte";
  import ControllerPanel from "./ControllerPanel.svelte";
  import HotspotPanel from "./HotspotPanel.svelte";
  import OpenSlidesPanel from "./OpenSlidesPanel.svelte";

  interface Props {
    conn: HostConnection;
  }
  let { conn }: Props = $props();

  const ROLES: Role[] = ["admin", "operator", "presenter", "stage_viewer"];
  const QR_CHOICE_KEY = "midnightsnack.qrChoice";

  const links = $derived(conn.pairing?.links ?? []);
  const linkId = (l: JoinLink) => `${l.kind}:${l.label}`;
  let choice = $state<string | null>(readChoice());
  function readChoice(): string | null {
    try {
      return localStorage.getItem(QR_CHOICE_KEY);
    } catch {
      return null;
    }
  }
  function choose(id: string) {
    choice = id;
    try {
      localStorage.setItem(QR_CHOICE_KEY, id);
    } catch {
      // Not remembered; fine.
    }
  }
  // The chosen link while it exists, otherwise the host's preferred one.
  const selected = $derived(links.find((l) => linkId(l) === choice) ?? links[0] ?? null);
  function linkLabel(l: JoinLink): string {
    const address = l.kind === "relay" ? "" : ` (${new URL(l.url).hostname})`;
    return t(`connect.link_${l.kind}`, { label: l.label }) + address;
  }

  let qr = $state<string | null>(null);
  $effect(() => {
    const url = selected?.url;
    if (!url) return;
    host.qrSvg(url).then((svg) => {
      // Generated locally from a URL we built; render as an image to avoid injecting markup.
      qr = `data:image/svg+xml;base64,${btoa(svg)}`;
    });
  });

  let approveRole = $state<Record<string, Role>>({});
  const remotes = $derived(conn.devices.filter((d) => !d.local && !d.api_key));
  const offline = $derived(remotes.filter((d) => !d.connected).length);

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

  async function forgetOffline() {
    const ok = await ask(t("devices.forget_offline_question", { n: offline }), {
      title: t("devices.forget_offline"),
      kind: "warning",
    });
    if (ok) conn.action({ action: "forget_offline_devices" });
  }

  function setAutoApprove(value: string) {
    conn.action({ action: "set_auto_approve", role: value === "" ? null : (value as Role) });
  }

  let renaming = $state<string | null>(null);
  function rename(d: DeviceInfo, name: string) {
    renaming = null;
    const clean = name.trim();
    if (clean && clean !== d.name) {
      conn.action({ action: "rename_device", device_id: d.id, name: clean });
    }
  }

  const ago = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
  function lastSeen(ms: number | null): string {
    if (ms === null) return t("devices.never_seen");
    const minutes = Math.round((ms - conn.hostNow()) / 60_000);
    const when =
      Math.abs(minutes) < 60
        ? ago.format(minutes, "minute")
        : Math.abs(minutes) < 48 * 60
          ? ago.format(Math.round(minutes / 60), "hour")
          : ago.format(Math.round(minutes / 1440), "day");
    return t("devices.last_seen", { when });
  }

  function connectionLabel(d: DeviceInfo): string {
    const via = d.path ? t(`devices.via_${d.path}`) : t("status.connected");
    const where = d.address ? ` · ${d.address}` : "";
    const latency = d.latency_ms != null ? ` · ${t("devices.latency", { ms: d.latency_ms })}` : "";
    return via + where + latency;
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
      {#if links.length > 1}
        <label class="field">
          <span>{t("connect.qr_for")}</span>
          <select
            value={selected ? linkId(selected) : ""}
            onchange={(e) => choose(e.currentTarget.value)}
          >
            {#each links as l (linkId(l))}
              <option value={linkId(l)}>{linkLabel(l)}</option>
            {/each}
          </select>
        </label>
      {/if}
      {#if selected?.kind === "https"}
        <p class="muted">{t("connect.https_hint")}</p>
      {:else if selected?.kind === "relay"}
        <p class="muted">{t("connect.relay_hint")}</p>
      {/if}
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
              <span class="muted">{p.via_relay ? t("devices.via_relay") : p.address}</span>
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
          <li data-device={d.id}>
            <span class="dot" class:on={d.connected} aria-hidden="true"></span>
            <div class="who">
              {#if renaming === d.id}
                <input
                  class="rename"
                  value={d.name}
                  maxlength="64"
                  aria-label={t("devices.rename")}
                  onkeydown={(e) => {
                    if (e.key === "Enter") rename(d, e.currentTarget.value);
                    if (e.key === "Escape") renaming = null;
                  }}
                  onblur={(e) => rename(d, e.currentTarget.value)}
                  {@attach (el) => el.focus()}
                />
              {:else}
                <button class="name" title={t("devices.rename")} onclick={() => (renaming = d.id)}
                  >{d.name}</button
                >
              {/if}
              <span class="muted">
                {d.connected ? connectionLabel(d) : lastSeen(d.last_seen_ms)}
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
      <div class="row">
        {#if offline > 0}
          <Button onclick={forgetOffline}>{t("devices.forget_offline")}</Button>
        {/if}
        <Button variant="danger" onclick={disconnectAll}>{t("devices.disconnect_all")}</Button>
      </div>
    {/if}

    <ConnectivityPanel {conn} />
    <OpenSlidesPanel {conn} />

    {#if !isController()}
      <HotspotPanel />
      <ControllerPanel />
    {/if}
  </div>
</div>

<style>
  .connect {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 10px;
  }
  .pair {
    display: flex;
    flex-wrap: wrap;
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
  .pending .who {
    flex-basis: 100%;
  }
  .name {
    all: unset;
    cursor: text;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name:focus-visible {
    outline: 2px solid var(--ms-accent);
    outline-offset: 2px;
  }
  .rename {
    font: inherit;
    font-weight: 600;
    padding: 2px 4px;
    background: var(--ms-surface-2);
    color: var(--ms-text);
    border: 1px solid var(--ms-border);
    border-radius: var(--ms-radius-sm);
    min-width: 0;
  }
  .muted {
    color: var(--ms-text-muted);
    font-size: 0.85rem;
    margin: 0;
  }
  .dot {
    width: 10px;
    height: 10px;
    flex: none;
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
    min-width: 0;
  }
  .auto,
  .field {
    display: grid;
    gap: 4px;
    font-size: 0.85rem;
    color: var(--ms-text-muted);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
</style>
