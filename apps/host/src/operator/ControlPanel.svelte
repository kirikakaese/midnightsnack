<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
<!-- Control surfaces: MIDI controllers (with learn), the OSC server and API keys; the language. -->
<script lang="ts">
  import type { Action, MidiBinding, MidiTrigger } from "@midnightsnack/protocol";
  import { Button, LANGUAGES, t, type HostConnection } from "@midnightsnack/ui";
  import { onDestroy } from "svelte";
  import { host, onMidiPress, type MidiSettings } from "../lib/host";
  import ApiKeysPanel from "./ApiKeysPanel.svelte";
  import { isController } from "../lib/mode";

  interface Props {
    conn: HostConnection;
  }
  let { conn }: Props = $props();

  // Actions a MIDI button can trigger, by a stable key.
  const SIMPLE = [
    "go",
    "next",
    "prev",
    "next_cue",
    "prev_cue",
    "toggle_blackout",
    "toggle_freeze",
    "toggle_logo",
    "panic",
    "clear_drawing",
    "timer_start",
    "timer_pause",
    "timer_reset",
    "countdown_start",
    "countdown_pause",
    "countdown_reset",
    "media_play",
    "media_pause",
    "media_restart",
  ] as const satisfies Action["action"][];
  const choices = $derived([
    ...SIMPLE.map((a) => ({ key: a, label: t(`control.action.${a}`) })),
    ...(conn.show?.overlays ?? []).map((o) => ({
      key: `toggle_overlay:${o.id}`,
      label: t("control.action.toggle_overlay", { name: o.name }),
    })),
  ]);

  function keyOf(a: Action): string {
    return a.action === "toggle_overlay" ? `toggle_overlay:${a.overlay_id}` : a.action;
  }
  function actionOf(key: string): Action {
    if (key.startsWith("toggle_overlay:")) {
      return { action: "toggle_overlay", overlay_id: key.slice("toggle_overlay:".length) };
    }
    return { action: key } as Action;
  }
  function triggerLabel(tr: MidiTrigger): string {
    const kind = tr.kind === "note" ? t("control.midi_note") : t("control.midi_cc");
    return `${kind} ${tr.number} · ${t("control.midi_channel", { n: tr.channel + 1 })}`;
  }

  let midi = $state<MidiSettings>({ enabled: true, bindings: [] });
  let ports = $state<string[]>([]);
  let learnAction = $state("next");
  let learning = $state(false);
  let learnFailed = $state(false);
  let lastPress = $state<MidiTrigger | null>(null);

  async function refresh() {
    [midi, ports] = await Promise.all([host.midiSettings(), host.midiPorts()]);
  }
  $effect(() => {
    if (!isController()) refresh();
  });
  const stop = onMidiPress((tr) => (lastPress = tr));
  onDestroy(stop);

  async function save(next: MidiSettings) {
    midi = next;
    await host.setMidiSettings(next);
  }

  async function learn() {
    learning = true;
    learnFailed = false;
    const trigger = await host.midiLearn();
    learning = false;
    if (!trigger) {
      learnFailed = true;
      return;
    }
    const bindings: MidiBinding[] = [
      ...midi.bindings.filter(
        (b) =>
          !(
            b.trigger.kind === trigger.kind &&
            b.trigger.channel === trigger.channel &&
            b.trigger.number === trigger.number
          ),
      ),
      { trigger, action: actionOf(learnAction) },
    ];
    await save({ ...midi, bindings });
  }

  let language = $state("");
  host
    .language()
    .then((l) => (language = l ?? ""))
    .catch(() => {});
  function setLanguage(value: string) {
    language = value;
    host.setLanguage(value || null);
  }

  const osc = $derived(conn.control?.osc ?? { enabled: false, port: 4748 });
  let oscPort = $derived(String(osc.port));
  function setOsc(enabled: boolean, port: number) {
    if (!Number.isInteger(port) || port < 1 || port > 65535) return;
    conn.action({ action: "configure_osc", osc: { enabled, port } });
  }
</script>

<div class="control">
  {#if !isController()}
    <section aria-label={t("control.midi")}>
      <div class="head">
        <h3>{t("control.midi")}</h3>
        <Button variant="ghost" onclick={refresh} aria-label={t("control.refresh")}>⟳</Button>
      </div>
      <label class="check">
        <input
          type="checkbox"
          checked={midi.enabled}
          onchange={(e) => save({ ...midi, enabled: e.currentTarget.checked })}
        />
        {t("control.midi_enabled")}
      </label>
      <p class="muted">
        {ports.length
          ? t("control.midi_ports", { ports: ports.join(", ") })
          : t("control.midi_none")}
      </p>
      {#if lastPress}
        <p class="muted" aria-live="polite">
          {t("control.midi_last", { what: triggerLabel(lastPress) })}
        </p>
      {/if}

      {#if midi.bindings.length}
        <ul class="list">
          {#each midi.bindings as b, i (i)}
            <li>
              <span class="trigger">{triggerLabel(b.trigger)}</span>
              <select
                value={keyOf(b.action)}
                aria-label={t("control.action_label")}
                onchange={(e) =>
                  save({
                    ...midi,
                    bindings: midi.bindings.map((x, j) =>
                      j === i ? { ...x, action: actionOf(e.currentTarget.value) } : x,
                    ),
                  })}
              >
                {#each choices as c (c.key)}
                  <option value={c.key}>{c.label}</option>
                {/each}
              </select>
              <Button
                variant="ghost"
                aria-label={t("control.remove_binding")}
                onclick={() => save({ ...midi, bindings: midi.bindings.filter((_, j) => j !== i) })}
                >✕</Button
              >
            </li>
          {/each}
        </ul>
      {/if}

      <div class="row">
        <select bind:value={learnAction} aria-label={t("control.action_label")}>
          {#each choices as c (c.key)}
            <option value={c.key}>{c.label}</option>
          {/each}
        </select>
        <Button variant="go" disabled={learning} onclick={learn}>
          {learning ? t("control.learning") : t("control.learn")}
        </Button>
      </div>
      {#if learnFailed}<p class="warn">{t("control.learn_timeout")}</p>{/if}
    </section>
  {/if}

  <section aria-label={t("control.osc")}>
    <h3>{t("control.osc")}</h3>
    <label class="check">
      <input
        type="checkbox"
        checked={osc.enabled}
        onchange={(e) => setOsc(e.currentTarget.checked, osc.port)}
      />
      {t("control.osc_enabled")}
    </label>
    <label class="row">
      <span>{t("control.osc_port")}</span>
      <input
        type="number"
        min="1"
        max="65535"
        bind:value={oscPort}
        onchange={() => setOsc(osc.enabled, Number(oscPort))}
      />
    </label>
    <p class="muted" role="status">
      {#if conn.control?.osc_listening}
        {conn.control.api_local_only
          ? t("control.osc_listening_local", { port: conn.control.osc_listening })
          : t("control.osc_listening", { port: conn.control.osc_listening })}
      {:else if osc.enabled}
        <span class="warn">{t("control.osc_failed")}</span>
      {:else}
        {t("control.osc_off")}
      {/if}
    </p>
  </section>

  <ApiKeysPanel {conn} />

  <section aria-label={t("settings.language")}>
    <h3>{t("settings.language")}</h3>
    <select value={language} onchange={(e) => setLanguage(e.currentTarget.value)}>
      <option value="">{t("settings.language_system")}</option>
      {#each LANGUAGES as l (l.code)}
        <option value={l.code} lang={l.code}>{l.name}</option>
      {/each}
    </select>
    <p class="muted">{t("settings.language_hint")}</p>
  </section>
</div>

<style>
  .control {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 14px;
  }
  section {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 8px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h3 {
    margin: 4px 0 0;
    font-size: 0.75rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ms-text-muted);
  }
  .muted {
    margin: 0;
    font-size: 0.8rem;
    color: var(--ms-text-muted);
  }
  .warn {
    margin: 0;
    color: var(--ms-warn);
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
  }
  li {
    display: grid;
    grid-template-columns: 1fr auto auto;
    gap: 6px;
    align-items: center;
  }
  .trigger {
    font-family: var(--ms-font-mono);
    font-size: 0.8rem;
  }
  li select {
    max-width: 150px;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .row input {
    width: 90px;
  }
  .check {
    display: flex;
    gap: 6px;
    align-items: center;
  }
</style>
