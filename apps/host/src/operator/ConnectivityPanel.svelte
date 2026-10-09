<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- How remotes reach this host: network interfaces, HTTPS on the LAN and the optional relay. -->
<script lang="ts">
  import { Button, t, type HostConnection } from "@midnightsnack/ui";
  import { ask } from "@tauri-apps/plugin-dialog";

  interface Props {
    conn: HostConnection;
  }
  let { conn }: Props = $props();

  const info = $derived(conn.connectivity);
  const relay = $derived(info?.relay ?? null);

  // Relay form; reset from the host's settings whenever they change.
  let relayUrl = $derived(relay?.url ?? "");
  let relayToken = $state("");
  let relayError = $state<string | null>(null);

  async function saveRelay(enabled: boolean) {
    relayError = null;
    const url = relayUrl.trim();
    if (enabled && !/^https?:\/\/[^/?#]+/.test(url)) {
      relayError = t("relay.invalid_url");
      return;
    }
    const err = await conn.action({
      action: "configure_relay",
      enabled,
      url,
      // Empty field keeps the stored token.
      access_token: relayToken.trim() ? relayToken.trim() : null,
    });
    if (err) relayError = t(`error.${err}`);
    else relayToken = "";
  }

  async function removeToken() {
    await conn.action({
      action: "configure_relay",
      enabled: relay?.enabled ?? false,
      url: relayUrl.trim(),
      access_token: "",
    });
  }

  async function resetIdentity() {
    const ok = await ask(t("relay.reset_question"), {
      title: t("relay.reset"),
      kind: "warning",
    });
    if (ok) conn.action({ action: "reset_relay_identity" });
  }

  const relayState = $derived.by(() => {
    if (!relay) return "";
    if (relay.state === "failed" && relay.error) return t(`relay.error.${relay.error}`);
    return t(`relay.state.${relay.state}`);
  });
  const relayLine = $derived(
    relay?.state === "connected"
      ? `${relayState} · ${t("relay.remotes", { n: relay.remotes })}`
      : relayState,
  );
</script>

{#if info}
  <section class="ms-form" aria-label={t("connectivity.title")}>
    <h3>{t("connectivity.network")}</h3>
    <ul class="ifaces">
      {#each info.interfaces as i (i.name + i.address)}
        <li>
          <code>{i.address}:{info.port}</code>
          <span class="muted">{i.name}</span>
          {#if i.hotspot}<span class="badge">{t("connectivity.hotspot")}</span>{/if}
        </li>
      {/each}
    </ul>

    <h3>{t("connectivity.https")}</h3>
    <label class="check">
      <input
        type="checkbox"
        checked={info.https.enabled}
        onchange={(e) => conn.action({ action: "set_https", on: e.currentTarget.checked })}
      />
      {t("connectivity.https_enable")}
    </label>
    <p class="hint">{t("connectivity.https_hint")}</p>
    {#if info.https.enabled}
      {#if info.https.port}
        <p class="hint">{t("connectivity.https_port", { port: info.https.port })}</p>
        <div class="field">
          <span>{t("connectivity.fingerprint")}</span>
          <code class="fingerprint">{info.https.fingerprint}</code>
        </div>
        <div class="row">
          <Button onclick={() => conn.action({ action: "renew_certificate" })}
            >{t("connectivity.renew")}</Button
          >
        </div>
      {:else}
        <p class="warn">{t("connectivity.https_failed")}</p>
      {/if}
    {/if}

    {#if relay}
      <h3>{t("relay.title")}</h3>
      <p class="hint">{t("relay.hint")}</p>
      <label>
        <span>{t("relay.url")}</span>
        <input
          bind:value={relayUrl}
          type="url"
          placeholder="https://relay.example.org"
          spellcheck="false"
          autocomplete="off"
        />
      </label>
      <label>
        <span>{t("relay.access_token")}</span>
        <input
          bind:value={relayToken}
          type="password"
          autocomplete="off"
          placeholder={relay.has_access_token ? t("relay.token_kept") : t("relay.token_none")}
        />
      </label>
      <div class="row">
        {#if relay.enabled}
          <Button variant="go" onclick={() => saveRelay(true)}>{t("relay.save")}</Button>
          <Button onclick={() => saveRelay(false)}>{t("relay.disconnect")}</Button>
        {:else}
          <Button variant="go" onclick={() => saveRelay(true)}>{t("relay.connect")}</Button>
        {/if}
        {#if relay.has_access_token}
          <Button variant="ghost" onclick={removeToken}>{t("relay.remove_token")}</Button>
        {/if}
      </div>
      {#if relayError}<p class="error" role="alert">{relayError}</p>{/if}
      {#if relay.enabled}
        <p
          class="status"
          class:ok={relay.state === "connected"}
          class:bad={relay.state === "failed"}
        >
          {relayLine}
        </p>
      {/if}
      <details>
        <summary>{t("relay.identity")}</summary>
        <p class="hint">{t("relay.host_id", { id: relay.host_id })}</p>
        <Button variant="danger" onclick={resetIdentity}>{t("relay.reset")}</Button>
      </details>
    {/if}
  </section>
{/if}

<style>
  .ifaces {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
  }
  .ifaces li {
    display: flex;
    gap: 8px;
    align-items: baseline;
    flex-wrap: wrap;
  }
  .muted {
    color: var(--ms-text-muted);
    font-size: 0.85rem;
  }
  .badge {
    font-size: 0.7rem;
    padding: 1px 6px;
    border-radius: 999px;
    background: var(--ms-go);
    color: var(--ms-go-text);
  }
  .fingerprint {
    font-size: 0.75rem;
    word-break: break-all;
    color: var(--ms-text);
  }
  .status {
    margin: 0;
    font-size: 0.85rem;
  }
  .status.ok {
    color: var(--ms-go);
  }
  .status.bad,
  .warn {
    color: var(--ms-warn);
    margin: 0;
  }
  .error {
    color: var(--ms-danger);
    margin: 0;
  }
  details summary {
    cursor: pointer;
    color: var(--ms-text-muted);
    font-size: 0.85rem;
  }
  details[open] {
    display: grid;
    gap: 8px;
  }
</style>
