<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- The connection to an OpenSlides 4 instance: server, account, meeting, status. -->
<script lang="ts">
  import { Button, t, type HostConnection } from "@midnightsnack/ui";

  interface Props {
    conn: HostConnection;
  }
  let { conn }: Props = $props();

  const status = $derived(conn.openslidesStatus);
  // Form fields, reset from the host's settings whenever they change.
  let url = $derived(status?.url ?? "");
  let username = $derived(status?.username ?? "");
  let password = $state("");
  let error = $state<string | null>(null);

  async function save(enabled: boolean, meetingId: number | null = status?.meeting_id ?? null) {
    error = null;
    if (enabled && !/^https?:\/\/[^/?#]+/.test(url.trim())) {
      error = t("openslides.invalid_url");
      return;
    }
    const err = await conn.action({
      action: "configure_open_slides",
      enabled,
      url: url.trim(),
      username: username.trim(),
      // Empty field keeps the stored password.
      password: password ? password : null,
      meeting_id: meetingId,
    });
    if (err) error = t(`error.${err}`);
    else password = "";
  }

  const stateText = $derived.by(() => {
    if (!status) return "";
    if (status.state === "failed" && status.error) return t(`openslides.error.${status.error}`);
    return t(`openslides.state.${status.state}`);
  });
</script>

{#if status}
  <section class="ms-form" aria-label={t("openslides.title")}>
    <h3>{t("openslides.title")}</h3>
    <p class="hint">{t("openslides.hint")}</p>
    <label>
      <span>{t("openslides.url")}</span>
      <input
        bind:value={url}
        type="url"
        placeholder="https://openslides.example.org"
        spellcheck="false"
        autocomplete="off"
      />
    </label>
    <div class="row">
      <label>
        <span>{t("openslides.username")}</span>
        <input bind:value={username} autocomplete="off" placeholder={t("openslides.public")} />
      </label>
      <label>
        <span>{t("openslides.password")}</span>
        <input
          bind:value={password}
          type="password"
          autocomplete="off"
          placeholder={status.has_password ? t("relay.token_kept") : ""}
        />
      </label>
    </div>
    <div class="row">
      {#if status.enabled}
        <Button variant="go" onclick={() => save(true)}>{t("relay.save")}</Button>
        <Button onclick={() => save(false)}>{t("relay.disconnect")}</Button>
      {:else}
        <Button variant="go" onclick={() => save(true)}>{t("relay.connect")}</Button>
      {/if}
    </div>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    {#if status.enabled}
      <p
        class="status"
        class:ok={status.state === "connected"}
        class:bad={status.state === "failed"}
      >
        {stateText}
      </p>
      {#if status.meetings.length || status.meeting_id !== null}
        <label>
          <span>{t("openslides.meeting")}</span>
          <select
            value={status.meeting_id}
            onchange={(e) => {
              const v = e.currentTarget.value;
              save(true, v === "" ? null : Number(v));
            }}
          >
            <option value="">{t("openslides.choose_meeting")}</option>
            {#each status.meetings as m (m.id)}
              <option value={m.id}>{m.name}</option>
            {/each}
            {#if status.meeting_id !== null && !status.meetings.some((m) => m.id === status.meeting_id)}
              <option value={status.meeting_id}>#{status.meeting_id}</option>
            {/if}
          </select>
        </label>
      {:else if !status.username}
        <label>
          <span>{t("openslides.meeting_id")}</span>
          <input
            type="number"
            min="1"
            onchange={(e) => save(true, Number(e.currentTarget.value) || null)}
          />
        </label>
      {/if}
    {/if}
  </section>
{/if}

<style>
  .row label {
    flex: 1 1 10em;
  }
  .status {
    margin: 0;
    font-size: 0.85rem;
  }
  .status.ok {
    color: var(--ms-go);
  }
  .status.bad {
    color: var(--ms-warn);
  }
  .error {
    margin: 0;
    color: var(--ms-danger);
  }
</style>
