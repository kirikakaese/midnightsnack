# Getting started

## 1. Build a show

A **show** is a list of **cues**. In the operator window use:

- **+ Files** — PDF decks and images (PNG, JPEG, GIF, WebP, BMP, TIFF). Each file becomes a cue.
- **+ Folder** — a folder of images becomes one cue with one slide per image, in natural order
  (`2.png` before `10.png`).
- **+ Blank** — a black slide, e.g. for breaks.

Click a cue to select it and edit it in the **Cue** tab; double-click it or press ▶ to send it
live. Drag cues (or use ↑/↓) to reorder, ✎ to rename, click the color bar to tag a cue, ✕ to
remove it. Video, audio, text and timer cues are described in [media.md](media.md).

### Speaker notes

Notes come from the PDF's comment (sticky note) annotations, one per page. Alternatively put a
text file next to the deck named `<deck>.notes.md` (or `.notes.txt`) and separate the slides
with a line containing only `---`:

```text
Welcome everyone, introduce the sponsors.
---
Second slide: keep it short.
```

A sidecar file wins over annotations. Annotations are never drawn on the projector.

## 2. Put it on the projector

In the **Output** panel choose the projector's display and click **Open output**. The output
covers that display without borders and without a mouse cursor, and never takes keyboard focus.
Your screen is kept awake while the output is open. With a single screen, tick
**Windowed (rehearsal)** to see the output in a normal window.

midnightsnack remembers the display and reopens the output there next time if it is connected.

## 3. Run the show

| Button / key                          | What it does                                             |
| ------------------------------------- | -------------------------------------------------------- |
| **GO** / **NEXT**, Space, →, PageDown | Start the show / next slide (continues into the next cue) |
| **PREV**, ←, PageUp                   | Previous slide                                           |
| **CUE ⏭ / ⏮ CUE**, Shift+→ / Shift+← | Jump to the next cue / start of the current or previous cue |
| Double-click a cue, ▶, or click a slide thumbnail | Jump there                               |
| **BLACKOUT**, B or `.`                | Black screen (toggle)                                    |
| **FREEZE**, F                         | Keep the audience on the current slide while you move on |
| **LOGO**, L                           | Show the logo screen (toggle)                            |
| **PANIC**, Esc                        | Logo screen immediately and release freeze               |

The left monitor (**Program**) always shows exactly what the audience sees, including blackout
and logo. The right monitor shows what comes **next**. While frozen, a small monitor shows
where you are behind the freeze.

The show timer starts with the first slide; pause or reset it in the top bar. The slide timer
restarts on every slide.

## 4. Save

**Save** writes a `.msnack` file with all media embedded, so it can be copied to another
computer. **Save linked…** writes a small file that refers to your media in place. Your work is
also autosaved continuously: after a crash, midnightsnack reopens the show at the slide you were
on.
