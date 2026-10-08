<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import { t } from "@midnightsnack/ui";
  import { clearJoinUrl, joinTokenFromUrl, storage } from "./lib/storage";
  import Join from "./views/Join.svelte";
  import Remote from "./views/Remote.svelte";

  const joinToken = joinTokenFromUrl();
  // A fresh QR scan always starts a new pairing, even if this browser was paired before.
  let token = $state<string | null>(joinToken ? null : storage.token());

  function paired(t: string) {
    clearJoinUrl();
    token = t;
  }

  function unpaired() {
    storage.setToken(null);
    token = null;
  }
</script>

<svelte:head><title>{t("remote.title")}</title></svelte:head>

{#if token}
  {#key token}
    <Remote {token} onunpaired={unpaired} />
  {/key}
{:else if joinToken}
  <Join {joinToken} onpaired={paired} />
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
