<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  import { Panel, StatusDot, t } from "@midnightsnack/ui";
</script>

<main>
  <h1>{t("app.name")}</h1>
  <Panel>
    <StatusDot status="pending" label={t("status.connecting")} />
    <p>{t("remote.placeholder")}</p>
  </Panel>
</main>

<style>
  main {
    display: grid;
    gap: var(--ms-gap);
    padding: max(var(--ms-gap), env(safe-area-inset-top)) var(--ms-gap);
  }
  h1 {
    margin: 0;
    font-size: 1.2rem;
  }
</style>
