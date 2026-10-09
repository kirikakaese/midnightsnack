// SPDX-License-Identifier: GPL-3.0-or-later
// Bitfocus Companion module: buttons for midnightsnack (actions), button colors that follow the
// show (feedbacks) and text such as the live cue or the countdown (variables).
import {
  InstanceBase,
  InstanceStatus,
  combineRgb,
  type CompanionActionDefinitions,
  type CompanionFeedbackDefinitions,
  type CompanionStaticUpgradeScript,
  type DropdownChoice,
  type SomeCompanionConfigField,
} from "@companion-module/base";
import type { Action } from "@midnightsnack/protocol";
import { HostClient, type ClientStatus } from "./client.js";
import {
  SIMPLE_ACTIONS,
  VARIABLES,
  countdownOvertime,
  cueLive,
  goToCue,
  masterAction,
  masterActive,
  overlayAction,
  overlayVisible,
  variables,
  type Master,
  type Switch,
} from "./state.js";

interface Config {
  host: string;
  port: number;
  [key: string]: string | number;
}

interface Secrets {
  token: string;
  [key: string]: string;
}

export const UpgradeScripts: CompanionStaticUpgradeScript<Config, Secrets>[] = [];

const RED = combineRgb(200, 0, 0);
const BLUE = combineRgb(0, 90, 200);
const PURPLE = combineRgb(130, 60, 200);
const GREEN = combineRgb(0, 150, 60);
const WHITE = combineRgb(255, 255, 255);

const SWITCH_CHOICES: DropdownChoice[] = [
  { id: "toggle", label: "Toggle" },
  { id: "on", label: "On" },
  { id: "off", label: "Off" },
];

const SIMPLE_LABELS: Record<keyof typeof SIMPLE_ACTIONS, string> = {
  go: "Go",
  next: "Next slide",
  prev: "Previous slide",
  next_cue: "Next cue",
  prev_cue: "Previous cue (start of cue)",
  panic: "Panic (logo now)",
  clear_drawing: "Clear drawing",
  media_play: "Media: play",
  media_pause: "Media: pause",
  media_restart: "Media: restart",
  timer_start: "Slide timer: start",
  timer_pause: "Slide timer: pause",
  timer_reset: "Slide timer: reset",
  countdown_start: "Countdown: start",
  countdown_pause: "Countdown: pause",
  countdown_reset: "Countdown: reset",
};

export default class MidnightsnackInstance extends InstanceBase {
  #client: HostClient | null = null;
  #tick: ReturnType<typeof setInterval> | undefined;
  #overlays = "";

  async init(config: Config, _isFirstInit: boolean, secrets: Secrets): Promise<void> {
    this.setVariableDefinitions(
      Object.fromEntries(Object.entries(VARIABLES).map(([id, name]) => [id, { name }])),
    );
    this.#defineActionsAndFeedbacks();
    this.#start(config, secrets);
    // Timers tick between state messages.
    this.#tick = setInterval(() => this.#updateVariables(), 500);
  }

  async destroy(): Promise<void> {
    clearInterval(this.#tick);
    this.#client?.close();
    this.#client = null;
  }

  async configUpdated(config: Config, secrets: Secrets): Promise<void> {
    this.#client?.close();
    this.#start(config, secrets);
  }

  getConfigFields(): SomeCompanionConfigField[] {
    return [
      {
        type: "static-text",
        id: "info",
        width: 12,
        label: "Setup",
        value:
          "Create an API key in midnightsnack (Control tab → API keys) with the Operator role " +
          "and paste it below. If Companion runs on another computer, untick “Only from this " +
          "computer” there.",
      },
      {
        type: "textinput",
        id: "host",
        label: "Host address",
        width: 8,
        default: "127.0.0.1",
      },
      {
        type: "number",
        id: "port",
        label: "Port",
        width: 4,
        default: 4747,
        min: 1,
        max: 65535,
      },
      {
        type: "secret-text",
        id: "token",
        label: "API key",
        width: 12,
      },
    ];
  }

  #start(config: Config, secrets: Secrets): void {
    const host = String(config.host ?? "").trim();
    const token = String(secrets?.token ?? "").trim();
    if (!host || !token) {
      this.updateStatus(InstanceStatus.BadConfig, "Host address and API key are required");
      return;
    }
    const port = Number(config.port) || 4747;
    const url = `ws://${host.includes(":") && !host.startsWith("[") ? `[${host}]` : host}:${port}/api/v1/ws`;
    this.#client = new HostClient(url, token, {
      status: (s) => this.#onStatus(s),
      state: () => this.#onState(),
    });
    this.#client.connect();
  }

  #onStatus(status: ClientStatus): void {
    const map: Record<ClientStatus, [InstanceStatus, string | null]> = {
      connecting: [InstanceStatus.Connecting, null],
      ok: [InstanceStatus.Ok, null],
      disconnected: [InstanceStatus.Disconnected, "Host not reachable"],
      unauthorized: [InstanceStatus.AuthenticationFailure, "API key rejected (revoked?)"],
      incompatible: [InstanceStatus.ConnectionFailure, "Incompatible midnightsnack version"],
    };
    const [s, message] = map[status];
    this.updateStatus(s, message);
  }

  #onState(): void {
    // Overlay choices follow the show.
    const overlays = JSON.stringify(this.#client?.show?.overlays.map((o) => [o.id, o.name]) ?? []);
    if (overlays !== this.#overlays) {
      this.#overlays = overlays;
      this.#defineActionsAndFeedbacks();
    }
    this.#updateVariables();
    this.checkFeedbacks("master", "cue_live", "overlay_visible", "countdown_overtime");
  }

  #updateVariables(): void {
    const c = this.#client;
    if (!c) return;
    this.setVariableValues(variables(c.show, c.live, c.hostNow()));
  }

  async #send(action: Action | null): Promise<void> {
    if (!action || !this.#client) return;
    const error = await this.#client.action(action);
    if (error) this.log("warn", `midnightsnack refused ${action.action}: ${error}`);
  }

  #defineActionsAndFeedbacks(): void {
    const overlayChoices: DropdownChoice[] = (this.#client?.show?.overlays ?? []).map((o) => ({
      id: o.id,
      label: o.name,
    }));
    const masterChoices: DropdownChoice[] = [
      { id: "blackout", label: "Blackout" },
      { id: "freeze", label: "Freeze" },
      { id: "logo", label: "Logo" },
    ];

    const actions: CompanionActionDefinitions = {};
    for (const [id, action] of Object.entries(SIMPLE_ACTIONS)) {
      actions[id] = {
        name: SIMPLE_LABELS[id as keyof typeof SIMPLE_ACTIONS],
        options: [],
        callback: () => this.#send({ action } as Action),
      };
    }
    actions["master"] = {
      name: "Blackout / freeze / logo",
      options: [
        {
          type: "dropdown",
          id: "master",
          label: "Master",
          choices: masterChoices,
          default: "blackout",
        },
        { type: "dropdown", id: "mode", label: "Mode", choices: SWITCH_CHOICES, default: "toggle" },
      ],
      callback: (e) =>
        this.#send(masterAction(e.options.master as Master, e.options.mode as Switch)),
    };
    actions["go_to_cue"] = {
      name: "Go to cue",
      options: [
        { type: "number", id: "cue", label: "Cue number", default: 1, min: 1, max: 9999 },
        { type: "number", id: "slide", label: "Slide", default: 1, min: 1, max: 9999 },
      ],
      callback: (e) =>
        this.#send(
          goToCue(this.#client?.show ?? null, Number(e.options.cue), Number(e.options.slide)),
        ),
    };
    actions["overlay"] = {
      name: "Overlay",
      options: [
        {
          type: "dropdown",
          id: "overlay",
          label: "Overlay",
          choices: overlayChoices,
          default: overlayChoices[0]?.id ?? "",
        },
        { type: "dropdown", id: "mode", label: "Mode", choices: SWITCH_CHOICES, default: "toggle" },
      ],
      callback: (e) =>
        this.#send(overlayAction(String(e.options.overlay), e.options.mode as Switch)),
    };
    actions["stage_message"] = {
      name: "Message to stage",
      options: [{ type: "textinput", id: "text", label: "Message (empty clears it)", default: "" }],
      callback: (e) => {
        const text = String(e.options.text ?? "").trim();
        return this.#send({ action: "set_stage_message", text: text || null });
      },
    };
    this.setActionDefinitions(actions);

    const live = () => this.#client?.live ?? null;
    const feedbacks: CompanionFeedbackDefinitions = {
      master: {
        type: "boolean",
        name: "Blackout / freeze / logo active",
        defaultStyle: { bgcolor: RED, color: WHITE },
        options: [
          {
            type: "dropdown",
            id: "master",
            label: "Master",
            choices: masterChoices,
            default: "blackout",
          },
        ],
        callback: (f) => masterActive(live(), f.options.master as Master),
      },
      cue_live: {
        type: "boolean",
        name: "Cue is live",
        defaultStyle: { bgcolor: GREEN, color: WHITE },
        options: [
          { type: "number", id: "cue", label: "Cue number", default: 1, min: 1, max: 9999 },
        ],
        callback: (f) => cueLive(this.#client?.show ?? null, live(), Number(f.options.cue)),
      },
      overlay_visible: {
        type: "boolean",
        name: "Overlay shown",
        defaultStyle: { bgcolor: BLUE, color: WHITE },
        options: [
          {
            type: "dropdown",
            id: "overlay",
            label: "Overlay",
            choices: overlayChoices,
            default: overlayChoices[0]?.id ?? "",
          },
        ],
        callback: (f) => overlayVisible(live(), String(f.options.overlay)),
      },
      countdown_overtime: {
        type: "boolean",
        name: "Countdown in overtime",
        defaultStyle: { bgcolor: PURPLE, color: WHITE },
        options: [],
        callback: () => countdownOvertime(live(), this.#client?.hostNow() ?? Date.now()),
      },
    };
    this.setFeedbackDefinitions(feedbacks);
  }
}
