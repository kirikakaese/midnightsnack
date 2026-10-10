<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import { t } from "@midnightsnack/ui";
  import {
    clearJoinUrl,
    currentLink,
    handedOverToken,
    joinTokenFromUrl,
    makeTransport,
    storage,
  } from "./lib/storage";
  import Join from "./views/Join.svelte";
  import Remote from "./views/Remote.svelte";

  const link = currentLink();
  const joinToken = joinTokenFromUrl();
  // Switching between the LAN and the relay carries the token in the fragment. It never
  // replaces a stored token, so a crafted link cannot unpair this phone or swap its identity.
  const handover = handedOverToken();
  if (handover && !storage.token(link)) storage.setToken(link, handover);
  if (handover || (window.location.hash.includes("k=") && !joinToken)) clearJoinUrl();
  // A fresh QR scan always starts a new pairing, even if this browser was paired before.
  let token = $state<string | null>(joinToken ? null : storage.token(link));
  const usable = makeTransport(link) !== null;

  function paired(t: string) {
    clearJoinUrl();
    storage.setToken(link, t);
    token = t;
  }

  function unpaired() {
    storage.setToken(link, null);
    token = null;
  }
</script>

<svelte:head><title>{t("remote.title")}</title></svelte:head>

{#if !usable}
  <main class="scan">
    <h1>{t("app.name")}</h1>
    <p>{t("remote.link_incomplete")}</p>
  </main>
{:else if token}
  {#key token}
    <Remote {link} {token} onunpaired={unpaired} />
  {/key}
{:else if joinToken}
  <Join {link} {joinToken} onpaired={paired} />
{:else}
  <main class="scan">
    <h1>{t("app.name")}</h1>
    <p>{t("remote.scan_again")}</p>
  </main>
{/if}

<style>
  .scan {
    padding: 40px var(--ms-gap);
    text-align: center;
  }
</style>
