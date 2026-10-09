// SPDX-License-Identifier: GPL-3.0-or-later
// Whether this window controls another host (controller mode). Set once, before the operator
// view mounts; host-only features (file dialogs, output windows, MIDI) are hidden then.
let controller = false;

export function setControllerMode(on: boolean): void {
  controller = on;
}

export function isController(): boolean {
  return controller;
}
