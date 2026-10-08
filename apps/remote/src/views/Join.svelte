<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import type { ErrorCode } from "@midnightsnack/protocol";
  import { Button, Panel, t } from "@midnightsnack/ui";
  import { onDestroy } from "svelte";
  import { pairingStatus, requestPairing } from "../lib/api";
  import { guessDeviceName, storage } from "../lib/storage";

  interface Props {
    joinToken: string;
    onpaired: (token: string) => void;
  }
  let { joinToken, onpaired }: Props = $props();

  let name = $state(storage.deviceName() ?? guessDeviceName(t("remote.default_device_name")));
  let pin = $state("");
  let error = $state<ErrorCode | null>(null);
  let phase = $state<"form" | "waiting" | "denied">("form");
  let busy = $state(false);
  let poll: ReturnType<typeof setTimeout> | undefined;

  onDestroy(() => clearTimeout(poll));

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = null;
    storage.setDeviceName(name);
    const res = await requestPairing(joinToken, pin, name);
    busy = false;
    if (!res.ok) {
      error = res.code;
      return;
    }
    phase = "waiting";
    wait(res.requestId);
  }

  async function wait(id: string) {
    const status = await pairingStatus(id);
    if (status?.status === "approved") {
      storage.setToken(status.token);
      onpaired(status.token);
      return;
    }
    if (status?.status === "denied" || status === null) {
      phase = "denied";
      return;
    }
    poll = setTimeout(() => wait(id), 1000);
  }
</script>

<main class="join">
  <h1>{t("remote.join_title")}</h1>
  <Panel>
    {#if phase === "form"}
      <form onsubmit={submit}>
        <label>
          <span>{t("remote.device_name")}</span>
          <input bind:value={name} maxlength="64" autocomplete="off" required />
        </label>
        <label>
          <span>{t("remote.pin")}</span>
          <input
            class="pin"
            bind:value={pin}
            inputmode="numeric"
            pattern="[0-9]*"
            maxlength="6"
            autocomplete="one-time-code"
            required
          />
        </label>
        {#if error}<p class="error" role="alert">{t(`error.${error}`)}</p>{/if}
        <Button type="submit" variant="go" size="lg" disabled={busy || pin.length < 6}>
          {t("remote.join")}
        </Button>
      </form>
    {:else if phase === "waiting"}
      <p class="waiting" role="status">{t("remote.waiting")}</p>
    {:else}
      <p class="error" role="alert">{t("remote.denied")}</p>
      <p>{t("remote.scan_again")}</p>
    {/if}
  </Panel>
</main>

<style>
  .join {
    display: grid;
    gap: var(--ms-gap);
    padding: max(var(--ms-gap), env(safe-area-inset-top)) var(--ms-gap) var(--ms-gap);
    max-width: 480px;
    margin: 0 auto;
  }
  h1 {
    margin: 8px 0 0;
    font-size: 1.4rem;
  }
  form {
    display: grid;
    gap: 14px;
  }
  label {
    display: grid;
    gap: 6px;
    color: var(--ms-text-muted);
  }
  input {
    font: inherit;
    font-size: 1.1rem;
    padding: 12px;
    background: var(--ms-surface-2);
    color: var(--ms-text);
    border: 2px solid var(--ms-border);
    border-radius: var(--ms-radius-sm);
  }
  .pin {
    font-family: var(--ms-font-mono);
    font-size: 2rem;
    letter-spacing: 0.3em;
    text-align: center;
  }
  .error {
    color: var(--ms-danger);
    margin: 0;
  }
  .waiting {
    font-size: 1.1rem;
  }
</style>
