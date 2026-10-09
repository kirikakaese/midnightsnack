// SPDX-License-Identifier: GPL-3.0-or-later
// Typed wrappers around the host's Tauri commands.
import type { HostInfo, MidiBinding, MidiTrigger } from "@midnightsnack/protocol";
import { HostConnection } from "@midnightsnack/ui";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface ConnectionInfo {
  ws_url: string;
  http_base: string;
  token: string;
  output_id: string | null;
}

export interface DisplayInfo {
  name: string;
  width: number;
  height: number;
  x: number;
  y: number;
  scale: number;
  primary: boolean;
}

/** Where an output's window goes on this host (kept in host settings, not in the show). */
export interface OutputWindowState {
  display: string | null;
  windowed: boolean;
  /** Reopen on launch. */
  open: boolean;
  /** The window exists right now. */
  window_open: boolean;
}

export interface MidiSettings {
  enabled: boolean;
  bindings: MidiBinding[];
}

export interface DisplayStatus {
  displays: DisplayInfo[];
  /** Output ids whose remembered display is not connected. */
  missing: string[];
}

export const host = {
  info: () => invoke<HostInfo>("host_info"),
  connectionInfo: () => invoke<ConnectionInfo>("connection_info"),
  listDisplays: () => invoke<DisplayInfo[]>("list_displays"),
  displayStatus: () => invoke<DisplayStatus>("display_status"),
  outputWindows: () => invoke<Record<string, OutputWindowState>>("output_windows"),
  openOutput: (outputId: string, display: string | null, windowed: boolean) =>
    invoke<void>("open_output", { outputId, display, windowed }),
  closeOutput: (outputId: string) => invoke<void>("close_output", { outputId }),
  keymap: () => invoke<Record<string, string>>("keymap"),
  qrSvg: (text: string) => invoke<string>("qr_svg", { text }),
  openCaptureSettings: () => invoke<void>("open_capture_settings"),
  midiPorts: () => invoke<string[]>("midi_ports"),
  midiSettings: () => invoke<MidiSettings>("midi_settings"),
  setMidiSettings: (settings: MidiSettings) => invoke<void>("set_midi_settings", { settings }),
  /** Resolves with the next MIDI press, or `null` after 10 s. */
  midiLearn: () => invoke<MidiTrigger | null>("midi_learn"),
  uiReady: () => invoke<void>("ui_ready"),
};

/** Fired for every MIDI press (activity indicator). */
export function onMidiPress(cb: (trigger: MidiTrigger) => void): () => void {
  let unlisten: (() => void) | null = null;
  let cancelled = false;
  listen<MidiTrigger>("midi-press", (e) => cb(e.payload)).then((u) => {
    if (cancelled) u();
    else unlisten = u;
  });
  return () => {
    cancelled = true;
    unlisten?.();
  };
}

/** Fired by the host when displays are plugged or unplugged. */
export function onDisplaysChanged(cb: (status: DisplayStatus) => void): () => void {
  let unlisten: (() => void) | null = null;
  let cancelled = false;
  listen<DisplayStatus>("displays-changed", (e) => cb(e.payload)).then((u) => {
    if (cancelled) u();
    else unlisten = u;
  });
  return () => {
    cancelled = true;
    unlisten?.();
  };
}

/** Connects this window to the embedded server with its window-specific token. */
export async function connectToHost(): Promise<HostConnection> {
  const info = await host.connectionInfo();
  const conn = new HostConnection({
    wsUrl: info.ws_url,
    httpBase: info.http_base,
    token: info.token,
  });
  conn.connect();
  return conn;
}
