# Phones and tablets as remotes

Any phone, tablet or computer on the same network can control the show from its browser —
nothing to install.

## Pairing

1. In the operator window, the **Connect a remote** panel shows a QR code and a PIN.
2. Scan the QR code with the phone's camera and open the link.
3. Enter a device name and the PIN.
4. On the laptop, choose a role for the device and click **Approve**.

The phone remembers the pairing; reloading the page or reconnecting to Wi-Fi does not require
pairing again. Each QR code works once; it refreshes automatically after a phone has used it.

If the phone cannot open the page, check that it is on the same network as the laptop, or pick
another address under **QR code for**. Some guest networks block devices from talking to each
other — use the laptop's hotspot or a relay; see [connectivity.md](connectivity.md).

## Roles

| Role             | Can                                                                     |
| ---------------- | ----------------------------------------------------------------------- |
| **Admin**        | Everything the operator window can do except opening files on the laptop |
| **Operator**     | Run the show: next/previous, jump to cues, blackout, freeze, logo, media playback, overlays, countdown, stage messages, pointer and drawing |
| **Presenter**    | Next/previous within the current cue only; laser pointer and drawing; send files to the inbox; sees notes and timers |
| **Stage viewer** | Read-only stage display: current and next slide, notes, big timers       |

Change a device's role, rename it (click its name) or remove it in the **Devices** list, which
also shows how each device is connected. **Disconnect all remotes** forgets
every paired device and changes the PIN — use it if a phone is lost or you suspect misuse. API
keys (see [control.md](control.md)) are kept; revoke those one by one.

**Approve new devices automatically as…** skips the approval step for a given role (handy for
stage displays during rehearsals). The PIN is still required.

## Laser pointer and drawing

Presenters and operators get **Laser** and **Draw** buttons below the current slide. With one of
them on, the slide fills the width of the phone: touch it to move a red dot on the projector, or
draw with a finger in the chosen color. Drawings belong to the slide — they disappear when the
slide changes, or with **Clear drawing**. In the operator window the same buttons sit above the
program monitor, for drawing with the mouse.

## Sending files

**Send a file to the host** uploads a PDF, presentation, image, video or audio file from the
phone. It waits in the **Inbox** at the top of the operator's cue list until the operator adds
it (at the end, or after the live cue) or rejects it. Files from admins, and from everyone while
**Add files from all devices without asking** is on (Connect tab), are added right away. The
host refuses other file types and files over 2 GB.

## Stage view

Every remote can switch to **Stage view**: large current/next slides, notes, timers, the
countdown and messages from the operator, no controls. Stage viewers always see it.
