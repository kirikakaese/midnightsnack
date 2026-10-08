<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  type Status = "ok" | "pending" | "error";
  interface Props {
    status: Status;
    label: string;
  }
  let { status, label }: Props = $props();
</script>

<span class="ms-status" role="status">
  <span class="dot dot--{status}" aria-hidden="true"></span>
  {label}
</span>

<style>
  .ms-status {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 0.9rem;
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }
  .dot--ok {
    background: var(--ms-go);
  }
  .dot--pending {
    background: var(--ms-warn);
  }
  .dot--error {
    background: var(--ms-danger);
  }
</style>
