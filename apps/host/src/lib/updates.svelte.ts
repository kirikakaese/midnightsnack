// SPDX-License-Identifier: GPL-3.0-or-later
// The app's update state (Control tab → Updates), kept current by `update-status` events.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type UpdateInterval = "daily" | "weekly" | "monthly";

export interface UpdateSettings {
  automatically_check: boolean;
  interval: UpdateInterval;
  automatically_install: boolean;
  include_beta: boolean;
  last_check_ms: number | null;
}

export type UpdateStatus =
  | { state: "idle" }
  | { state: "checking" }
  | { state: "up_to_date" }
  | { state: "available"; version: string; notes: string }
  | { state: "downloading"; version: string; percent: number | null }
  | { state: "ready"; version: string }
  | { state: "installing"; version: string }
  | { state: "failed"; message: string };

export interface UpdateInfo {
  supported: boolean;
  /** Installed from a Linux package: the package manager updates it. */
  package_managed: boolean;
  current_version: string;
  settings: UpdateSettings;
  status: UpdateStatus;
}

class Updates {
  info = $state<UpdateInfo | null>(null);

  /** An update waits to be installed (shown as a badge on the Control tab). */
  get pending(): boolean {
    const s = this.info?.status.state;
    return s === "available" || s === "downloading" || s === "ready";
  }

  constructor() {
    invoke<UpdateInfo>("update_info")
      .then((i) => (this.info = i))
      .catch(() => {});
    listen<UpdateInfo>("update-status", (e) => (this.info = e.payload)).catch(() => {});
  }

  setSettings(settings: UpdateSettings): Promise<void> {
    return invoke("set_update_settings", { settings });
  }

  check(): Promise<string | null> {
    return invoke<string | null>("check_for_updates");
  }

  /** Rejects with `outputs_open` while an output window is open. */
  install(): Promise<void> {
    return invoke("install_update");
  }
}

export const updates = new Updates();
