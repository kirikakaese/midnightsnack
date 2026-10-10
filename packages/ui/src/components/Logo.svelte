<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<script lang="ts">
  interface Props {
    size?: string;
  }
  let { size = "20vmin" }: Props = $props();
</script>

<svg viewBox="0 0 512 512" style:width={size} style:height={size} role="img" aria-label="DECK">
  <path d="M310 96a168 168 0 1 0 106 298A150 150 0 0 1 310 96z" fill="#ffd447" />
  <rect x="96" y="356" width="216" height="40" rx="20" fill="#4f8cff" />
  <circle cx="368" cy="160" r="18" fill="#eef1f7" />
</svg>
