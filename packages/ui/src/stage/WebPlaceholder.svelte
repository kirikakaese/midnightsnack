<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Stand-in for a web page cue in monitors and on phones (the output shows the real page). -->
<script lang="ts">
  import { t } from "../i18n/index.svelte";

  interface Props {
    name: string;
    url: string;
    openslides: boolean;
    onready: () => void;
  }
  let { name, url, openslides, onready }: Props = $props();
  $effect(() => onready());
  const host = $derived.by(() => {
    try {
      return new URL(url).host;
    } catch {
      return url;
    }
  });
</script>

<div class="web">
  <span class="icon" aria-hidden="true">{openslides ? "◫" : "🌐"}</span>
  <span class="name">{name}</span>
  <span class="host">{openslides ? t("cue.kind.openslides") : host}</span>
</div>

<style>
  .web {
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
  .host {
    font-size: 7cqh;
    opacity: 0.7;
  }
</style>
