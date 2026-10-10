# Presentations, web pages and screen capture

## PowerPoint, Keynote and LibreOffice presentations

Add `.pptx`, `.ppt`, `.odp` or `.key` files with **+ Files** like any PDF. DECK converts
them to PDF in the background and the cue behaves exactly like a PDF deck; the cue list shows the
original file name.

- **Converter:** PPTX, PPT and ODP need [LibreOffice](https://www.libreoffice.org) installed
  (free). DECK finds it in the usual install location or on the `PATH`; set
  `MIDNIGHTSNACK_SOFFICE` to the `soffice` program to use another one. Keynote files need Keynote
  on a Mac. Without a converter you get a message saying what to install — or export the deck
  as PDF yourself.
- **Speaker notes** are read from PPTX and ODP files (one note per slide). For PPT and Keynote,
  export to PPTX or use a `<deck>.notes.md` file (see [getting-started.md](getting-started.md)).
- **Fidelity:** LibreOffice renders most decks well, but fonts that are not installed are
  substituted and animations become static slides. Check the result before the show; if it
  matters, export a PDF from the original application.
- Conversions are cached. If you edit the presentation, it is converted again the next time the
  show is opened.
- A saved `.msnack` bundle contains both the original and the converted PDF, so it plays on a
  computer without LibreOffice.

## Web pages

**+ Add… → Web page** shows a live web page full screen, e.g. a reveal.js deck, a live result
board or a video stream. Settings in the **Cue** tab:

| Setting                      | Meaning                                                            |
| ---------------------------- | ------------------------------------------------------------------ |
| **Page address**             | `http://` or `https://` address                                    |
| **Zoom**                     | 25–400 %                                                           |
| **Stay on this site**        | Links to other sites are blocked (the page cannot wander off)      |
| **Next/previous go to the page** | NEXT/PREV (and your clicker) send → / ← to the page instead of changing cues; use **CUE ⏭** to move on |
| **Keep logins and cookies**  | The page keeps its login between shows; otherwise it runs privately and forgets everything when it closes |

The page is loaded in the background while it is the next cue, so it appears without a loading
screen. It runs isolated from DECK: it cannot control the show or read your files.
Blackout, logo and test patterns cover web pages; overlays are not drawn on top of them. The
monitors in the operator window and on phones show a placeholder with the address, not the page.

## OpenSlides projector

To show agenda, motions and lists of speakers in the show's own look, see
[OpenSlides](openslides.md). **+ Add… → OpenSlides projector** is a web page cue preset for an
[OpenSlides](https://openslides.com) projector: paste the projector's address
(`https://…/<meeting>/projector/1`). The projector updates itself, so next/previous are not sent
to the page; logins are kept. If the projector is not public, open the output **windowed** once
(see [outputs.md](outputs.md)), log in on the page there, then move the output back to the
projector.

## Screen and window capture

**+ Add… → Screen or window capture** lists the screens and windows DECK can capture;
pick one. The cue shows the live picture on the outputs and the operator's monitors, and at a few
frames per second on thumbnails and phones. In the **Cue** tab you can change the source and the
frame rate (5–30 fps).

If the captured window is closed or the screen disappears, the output keeps the last picture and
the **Cue** tab says the source is gone; capture resumes when it is back.

- **macOS:** the first time, macOS asks for **Screen Recording** permission. If you declined, the
  picker offers **Open screen recording settings**: allow DECK under *Privacy &
  Security → Screen Recording*, then restart DECK.
- **Windows:** works without setup.
- **Linux:** works on X11 sessions. Wayland sessions are not supported yet; log in with an X11
  ("Xorg") session to capture.

Capturing costs CPU: on a typical laptop a full-HD screen runs at about 25–30 fps. Lower the
frame rate for static content such as a spreadsheet.
