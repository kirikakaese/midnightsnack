<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Controller mode: pair with another midnightsnack host and open its operator view here. -->
<script lang="ts">
  import type { ErrorCode } from "@midnightsnack/protocol";
  import { Button, t } from "@midnightsnack/ui";
  import { host, type FoundHost, type RemoteHost } from "../lib/host";

  let remotes = $state<RemoteHost[]>([]);
  let found = $state<FoundHost[] | null>(null);
  let searching = $state(false);
  let address = $state("");
  let pin = $state("");
  let deviceName = $state(t("controller.default_name"));
  let pairing = $state(false);
  let error = $state<string | null>(null);

  const refresh = async () => (remotes = await host.remoteHosts());
  $effect(() => {
    refresh();
  });

  async function search() {
    searching = true;
    found = await host.discoverHosts();
    searching = false;
  }

  async function pair(e: SubmitEvent) {
    e.preventDefault();
    error = null;
    pairing = true;
    try {
      const remote = await host.pairRemote(address, pin, deviceName);
      pin = "";
      await refresh();
      await host.openController(remote.id);
    } catch (code) {
      error = t(`error.${String(code) as ErrorCode}`);
    } finally {
      pairing = false;
    }
  }
</script>

<section class="controller" aria-label={t("controller.title")}>
  <h3>{t("controller.title")}</h3>
  <p class="muted">{t("controller.hint")}</p>

  {#if remotes.length}
    <ul class="list">
      {#each remotes as r (r.id)}
        <li>
          <div class="who">
            <strong>{r.name}</strong>
            <span class="muted">{r.base_url} · {t(`role.${r.role}`)}</span>
          </div>
          <Button variant="go" onclick={() => host.openController(r.id)}
            >{t("controller.open")}</Button
          >
          <Button
            variant="ghost"
            aria-label={t("controller.forget", { name: r.name })}
            onclick={async () => {
              await host.forgetRemote(r.id);
              await refresh();
            }}>✕</Button
          >
        </li>
      {/each}
    </ul>
  {/if}

  <div class="row">
    <Button disabled={searching} onclick={search}>
      {searching ? t("controller.searching") : t("controller.search")}
    </Button>
  </div>
  {#if found}
    {#if found.length === 0}
      <p class="muted">{t("controller.none_found")}</p>
    {:else}
      <ul class="list">
        {#each found as f (f.url)}
          <li>
            <button class="found" onclick={() => (address = f.url)}>
              <strong>{f.name}</strong>
              <span class="muted">{f.url}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}

  <form class="ms-form" onsubmit={pair}>
    <label>
      <span>{t("controller.address")}</span>
      <input bind:value={address} placeholder="192.168.1.20:4747" required />
    </label>
    <div class="row">
      <label>
        <span>{t("controller.pin")}</span>
        <input bind:value={pin} inputmode="numeric" maxlength="6" size="8" required />
      </label>
      <label class="grow">
        <span>{t("controller.device_name")}</span>
        <input bind:value={deviceName} />
      </label>
    </div>
    <Button type="submit" variant="go" disabled={pairing || !address.trim() || !pin.trim()}>
      {pairing ? t("controller.waiting") : t("controller.pair")}
    </Button>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  </form>
</section>

<style>
  .controller {
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
    gap: 6px;
    align-items: center;
  }
  .who {
    display: grid;
    flex: 1;
    min-width: 0;
  }
  .found {
    display: grid;
    flex: 1;
    text-align: left;
    padding: 6px 8px;
    border: 1px solid var(--ms-border);
    border-radius: var(--ms-radius-sm);
    background: var(--ms-surface-2);
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: end;
  }
  .grow {
    flex: 1;
  }
  .error {
    margin: 0;
    color: var(--ms-danger);
  }
</style>
