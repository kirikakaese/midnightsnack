# Getting started

## 1. Build a show

A **show** is a list of **cues**. In the operator window use:

- **+ Files** — PDF decks, presentations (PPTX, PPT, ODP, Keynote), images (PNG, JPEG, GIF,
  WebP, BMP, TIFF), video and audio. Each file becomes a cue.
- **+ Add… → Folder** — a folder of images becomes one cue with one slide per image, in natural
  order (`2.png` before `10.png`).
- **+ Add… → Blank** — a black slide, e.g. for breaks.
- **+ Add… → Text, Timer, Web page, OpenSlides projector, Screen or window capture** — see
  [media.md](media.md) and [sources.md](sources.md).

Click a cue to select it and edit it in the **Cue** tab; double-click it or press ▶ to send it
live. Drag cues (or use ↑/↓) to reorder, ✎ to rename, click the color bar to tag a cue, ✕ to
remove it (the tools appear when you point at a cue). Video, audio, text and timer cues are
described in [media.md](media.md); presentations, web pages and capture in
[sources.md](sources.md).

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

In the **Outputs** tab choose the projector's display for the **Main** output and click
**Open output**. The output covers that display without borders and without a mouse cursor, and
never takes keyboard focus. Your screen is kept awake while an output is open. With a single
screen, tick **Windowed (rehearsal)** to see the output in a normal window.

midnightsnack remembers the display and reopens the output there next time if it is connected.
More screens (a second room, a confidence monitor for the speaker) are described in
[outputs.md](outputs.md).

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
