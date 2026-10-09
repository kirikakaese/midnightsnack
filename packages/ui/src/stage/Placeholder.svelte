<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Stand-in for content this device cannot show (video or capture through the relay). -->
<script lang="ts">
  interface Props {
    icon: string;
    name: string;
    detail: string;
    onready: () => void;
  }
  let { icon, name, detail, onready }: Props = $props();
  $effect(() => onready());
</script>

<div class="placeholder">
  <span class="icon" aria-hidden="true">{icon}</span>
  <span class="name">{name}</span>
  <span class="detail">{detail}</span>
</div>

<style>
  .placeholder {
    position: absolute;
    inset: 0;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 3%;
    background: #10131b;
    color: #eef1f7;
    container-type: size;
    text-align: center;
    padding: 0 6%;
  }
  .icon {
    font-size: 26cqh;
    line-height: 1;
  }
  .name {
    font-size: 9cqh;
    max-width: 85cqw;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .detail {
    font-size: 6cqh;
    opacity: 0.7;
  }
</style>
