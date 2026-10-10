# OpenSlides

DECK can show an [OpenSlides](https://openslides.com) 4 meeting natively: the agenda,
motions, topics and lists of speakers appear in the show's own look on the beamer, the operator
monitors, stage displays and phones, and update live as the meeting runs in OpenSlides.
DECK only reads from OpenSlides; the meeting is still run in OpenSlides.

For an exact copy of the OpenSlides projector page instead, use the
[OpenSlides projector](sources.md#openslides-projector) web cue.

## Connecting

1. Open the **Connect** tab → **OpenSlides**.
2. Enter the address of the OpenSlides instance (e.g. `https://openslides.example.org`), and a
   username and password. Leave the username empty for meetings with public access.
3. Click **Connect**. When it says *Signed in*, choose the **Meeting**. (With public access, enter
   the meeting's number from its address, `…/<number>/…`.)

The status line shows *Connected* while data flows. If OpenSlides cannot be reached, the account
is refused or the meeting is gone, it says so and retries; slides keep showing the last data.

Use an account that sees what the audience may see: everything this account can read about the
agenda, motions and speakers is sent to all paired devices so they can draw the slides. The
password is stored on this computer only (readable only by your user account) and never sent to
remotes. It is not needed again while the session stays valid.

## OpenSlides cues

**+ Add… → OpenSlides: agenda, motion, speakers…** adds a cue showing:

| Choice                        | Shows                                                                 |
| ----------------------------- | --------------------------------------------------------------------- |
| **Agenda**                    | The agenda items (internal ones only if OpenSlides is set to show them on the projector), closed items struck through. 12 items per slide. |
| **Current list of speakers**  | The list of speakers of whatever the OpenSlides reference projector shows: the current speaker with speaking time, the next five and the last two. |
| **Follow an OpenSlides projector** | Whatever that projector (default: the reference projector) currently shows — a motion, topic, list of speakers or the agenda — and changes with it. Elections, votes, files and messages show a titled placeholder. |
| **Motion**                    | Number, title, submitters, state, text and reason; long motions are split into several slides. |
| **Topic**                     | Title and text.                                                       |
| **List of speakers of…**      | A specific list of speakers.                                          |

The slides use the show's text theme (font, colors, background); change it for one cue in the
**Cue** tab like for text slides. Slide numbers (*1 / 3*) appear when a cue has several slides.
Agenda and motion cues gain or lose slides when the content changes in OpenSlides.

Speaking times use the host's clock; if the clocks of the OpenSlides server and this computer
differ by a lot, the time is hidden rather than shown wrong.

## Limits

- Elections, polls, files, projector messages and countdowns are not drawn natively yet. Follow
  cues show their title; use the OpenSlides projector web cue for them.
- Motion texts are shown as plain paragraphs, headings and list items (no tables, line numbers or
  change recommendations).
- One meeting at a time.
