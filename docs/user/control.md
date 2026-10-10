# MIDI, OSC, Stream Deck and a second computer

Everything in this guide is set up in the **Control** tab of the operator window, except
controller mode (**Connect** tab). Every control surface acts with a role, exactly like a phone:
an operator key can run the show but not edit it.

## API keys

Bitfocus Companion, scripts using the HTTP API, and OSC senders on other computers authenticate
with an **API key**. Under **API keys**, enter a name (e.g. *Stream Deck*), pick a role
(usually *Operator*) and click **Create key**. Copy the key right away — it is shown only once.
Revoke a key with ✕; anything using it stops working immediately.

**Only from this computer** (on by default) accepts API keys and OSC only from software running
on the host itself. Untick it when Companion or another controller runs on a different
computer.

## Bitfocus Companion (Stream Deck)

The module lives in [`integrations/companion`](../../integrations/companion). Until it is
listed in Companion, load it as a developer module:

1. Build it: `pnpm --filter companion-module-midnightsnack build`.
2. In Companion's settings, set the *developer modules* folder to the repository's
   `integrations` folder and add a **DECK** connection.
3. Enter the host's address, port (`4747`) and an API key.

**Actions:** go, next/previous slide, next/previous cue, go to cue *n* (and slide), blackout /
freeze / logo (toggle, on, off), panic, overlays, clear drawing, media play/pause/restart, slide
timer, countdown, message to stage.
**Feedbacks:** blackout / freeze / logo active (red), cue *n* live (green), overlay shown
(blue), countdown in overtime (purple).
**Variables:** `$(deck:cue_name)`, `cue_number`, `slide`, `slide_count`, `slide_of`,
`next_name`, `show_timer`, `slide_timer`, `countdown`, `countdown_label`, `stage_message`,
`show_title`.

## MIDI controllers

Plug in a MIDI controller (pads, buttons, a keyboard) — DECK listens on every MIDI input
and notices controllers plugged in later.

1. Choose an action in the list under **MIDI** (e.g. *Next slide*).
2. Click **Learn** and press the button or pad on the controller within 10 seconds.
3. The binding appears in the list; change its action or remove it there.

Notes react when pressed; controllers (CC) when their value goes from below to above the middle,
so both buttons and faders work. **Last press** shows what the host received, which helps when a
controller does not seem to work. MIDI acts with the operator role.

## OSC

Tick **Accept OSC messages** and choose a UDP port (default `4748`). With **Only from this
computer** on, OSC listens on this computer only and needs no key. Otherwise senders on other
computers first send `/midnightsnack/auth <api key>`.

| Address                                          | Arguments    | Does                                    |
| ------------------------------------------------ | ------------ | --------------------------------------- |
| `/midnightsnack/go`                              |              | Start / next slide                      |
| `/midnightsnack/next`, `/midnightsnack/prev`     |              | Next / previous slide                   |
| `/midnightsnack/next_cue`, `/midnightsnack/prev_cue` |          | Next cue / start of the cue             |
| `/midnightsnack/cue/{n}`                         | `[slide]`    | Go to cue *n* (1-based)                 |
| `/midnightsnack/goto`                            | `cue slide`  | Go to cue and slide (1-based)           |
| `/midnightsnack/blackout`, `/freeze`, `/logo`    | `[0\|1]`     | Set, or toggle without argument         |
| `/midnightsnack/panic`                           |              | Logo now, release freeze                |
| `/midnightsnack/overlay/{id}`                    | `[0\|1]`     | Show / hide / toggle an overlay         |
| `/midnightsnack/timer/{start,pause,reset}`       |              | Slide timer                             |
| `/midnightsnack/countdown/{start,pause,reset}`   |              | Countdown                               |
| `/midnightsnack/countdown/set`                   | `seconds`    | Countdown duration                      |
| `/midnightsnack/media/{play,pause,restart}`      |              | Media on the output                     |
| `/midnightsnack/clear_drawing`                   |              | Remove drawings                         |
| `/midnightsnack/auth`                            | `key`        | Authenticate this sender                |
| `/midnightsnack/subscribe`                       | `[port]`     | Receive feedback (to this or another port) |
| `/midnightsnack/unsubscribe`                     |              | Stop feedback                           |

Feedback arrives as a bundle on every change (and every second while timers run):
`/midnightsnack/state/blackout`, `freeze`, `logo` (0/1), `cue/number`, `cue/name`, `slide`,
`slide_count`, `next/name`, `countdown`, `countdown/running`, `show_timer`, `slide_timer`.
Subscriptions last an hour; send `/subscribe` again to renew.

## HTTP API

For scripts. Send an API key as `Authorization: Bearer <key>`.

```sh
# Next slide
curl -X POST -H "Authorization: Bearer $KEY" -H "Content-Type: application/json" \
  -d '{"action":"next"}' http://127.0.0.1:4747/api/v1/action

# What is live, masters and timers
curl -H "Authorization: Bearer $KEY" http://127.0.0.1:4747/api/v1/state
```

`POST /api/v1/action` takes any action of the [protocol](../dev/protocol.md) the key's role
allows and answers `{}` or `{"code": "forbidden"}` (and similar). Uploads work with
`POST /api/v1/upload?name=<file name>` and the file as body.

## A second computer as controller

The host app can run the show of another DECK host — for example from a desk at the
back of the room while the laptop with the projector stays on stage.

1. On the second computer open the **Connect** tab → **Control another computer**.
2. **Find hosts on the network** lists hosts that advertise themselves; pick one, or type the
   address shown in the other host's Connect tab (e.g. `192.168.1.20:4747`).
3. Enter the other host's **PIN** and click **Pair**. Its operator approves the request (choose
   *Admin* to allow editing). Pairing by PIN always needs this approval.
4. A window with the full operator view of the other host opens. **Open** brings it back later.

In that window everything works as on the host itself, except what belongs to the other
computer: opening and saving show files under new names, choosing images, and placing output
windows. **+ Files** uploads files from this computer instead.
