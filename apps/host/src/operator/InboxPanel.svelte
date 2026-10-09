<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Files sent from devices, waiting to be added to the show or rejected. -->
<script lang="ts">
  import { Button, t, type HostConnection } from "@midnightsnack/ui";

  interface Props {
    conn: HostConnection;
  }
  let { conn }: Props = $props();

  function size(bytes: number): string {
    if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
    return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  }

  /** Right after the cue that is live, or at the end when nothing is live. */
  function afterCurrent(): number | null {
    const id = conn.live?.program?.cue_id;
    const i = conn.show?.cues.findIndex((c) => c.id === id) ?? -1;
    return i >= 0 ? i + 1 : null;
  }
</script>

{#if conn.inbox.length}
  <section class="inbox" aria-label={t("inbox.title")}>
    <h3>{t("inbox.title")} <span class="count">{conn.inbox.length}</span></h3>
    <ul>
      {#each conn.inbox as item (item.id)}
        <li>
          <div class="info">
            <span class="name">{item.file_name}</span>
            <span class="meta"
              >{t("inbox.from", { size: size(item.size), device: item.device_name })}</span
            >
          </div>
          <div class="actions">
            <Button
              variant="go"
              onclick={() =>
                conn.action({ action: "accept_upload", upload_id: item.id, at_index: null })}
              >{t("inbox.add")}</Button
            >
            {#if conn.live?.program}
              <Button
                onclick={() =>
                  conn.action({
                    action: "accept_upload",
                    upload_id: item.id,
                    at_index: afterCurrent(),
                  })}>{t("inbox.add_next")}</Button
              >
            {/if}
            <Button
              variant="ghost"
              onclick={() => conn.action({ action: "reject_upload", upload_id: item.id })}
              >{t("inbox.reject")}</Button
            >
          </div>
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .inbox {
    margin-bottom: 8px;
    padding: 8px;
    border: 1px solid var(--ms-accent);
    border-radius: var(--ms-radius-sm);
  }
  h3 {
    margin: 0 0 6px;
    font-size: 0.75rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ms-text-muted);
  }
  .count {
    color: var(--ms-accent);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  li {
    display: grid;
    gap: 4px;
  }
  .info {
    display: grid;
    min-width: 0;
  }
  .name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    font-size: 0.8rem;
    color: var(--ms-text-muted);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
</style>
