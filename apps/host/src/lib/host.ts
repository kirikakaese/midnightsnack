// SPDX-License-Identifier: GPL-3.0-or-later
// Typed wrappers around the host's Tauri commands.
import type { HostInfo } from "@midnightsnack/protocol";
import { HostConnection } from "@midnightsnack/ui";
import { invoke } from "@tauri-apps/api/core";

export interface ConnectionInfo {
  ws_url: string;
  http_base: string;
  token: string;
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

export interface OutputState {
  open: boolean;
  display: string | null;
  windowed: boolean;
}

export const host = {
  info: () => invoke<HostInfo>("host_info"),
  connectionInfo: () => invoke<ConnectionInfo>("connection_info"),
  listDisplays: () => invoke<DisplayInfo[]>("list_displays"),
  outputState: () => invoke<OutputState>("output_state"),
  openOutput: (display: string | null, windowed: boolean) =>
    invoke<void>("open_output", { display, windowed }),
  closeOutput: () => invoke<void>("close_output"),
  keymap: () => invoke<Record<string, string>>("keymap"),
  qrSvg: (text: string) => invoke<string>("qr_svg", { text }),
  uiReady: () => invoke<void>("ui_ready"),
};

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
