# Phase 2 — Media & live content

Goal: everything beyond static slides that a typical event needs — video and audio, text and
lyrics, countdowns, smooth transitions, overlays and a proper stage display.

## Tasks

### Model and engine (`crates/core`)
- [x] Media cues (video, audio): loop, start/end trim, volume, auto-advance at the end; shared
      playback timeline (play, pause, seek, restart) so every view shows the same position.
- [x] Text cues: slides from plain text, lyrics mode splits verses on blank lines, otherwise
      `---` separates slides; themes (font, size or auto-fit, colors, alignment, background
      color or image) with a show default and per-cue override.
- [x] Timer cues: countdown for a duration, countdown to a wall-clock time, count-up, clock;
      overtime color.
- [x] Global countdown (for overlays, stage display and remotes) with start/pause/reset.
- [x] Per-cue transition (cut or fade with duration) and a show default; auto-advance after a
      delay per slide.
- [x] Overlays defined in the show (lower third, logo bug, clock, ticker, timer) with position
      and style; visibility toggled live.
- [x] Custom logo screen image.
- [x] Stage messages from the operator to stage displays.
- [x] Permissions for all new actions; bundles carry new media (backgrounds, logos).

### Server
- [x] Media file endpoint with HTTP range requests (video seeking) behind the media key.
- [x] Auto-advance scheduler (slide delays and media end).
- [x] Output windows report media duration and end of playback (local only).

### Frontends
- [x] Output: video/audio playback synchronized to the host timeline, preloading the next media
      cue, transitions (GPU-friendly opacity), overlays layer, text and timer rendering with
      auto-fit; never shows controls, spinners or errors.
- [x] Operator: add video/audio, text/lyrics and timer cues; cue inspector (transition,
      auto-advance, media options, text editor, theme); media transport and progress; countdown
      and stage message controls; overlay manager with quick toggles; logo image.
- [x] Stage display: dedicated window on the host and a stage view on remotes — current/next,
      notes, big timers, countdown with overtime color, operator message.
- [x] Remote: media progress and transport, countdown and overlays for operators.

### Docs and tests
- [x] Codec matrix per OS webview (`docs/user/media.md`); media layer ADR (webview HTML5
      media behind an interface; libmpv later).
- [x] Unit tests for engine additions, server tests for range requests and new permissions,
      E2E for stage message, overlays and text cues on the remote.

## Acceptance criteria

1. An MP4 (H.264/AAC) plays fullscreen on the output, starts with the cue, can be paused,
   seeked and restarted from the operator and a phone, loops or advances at the end as set.
2. A lyrics text pasted into a new text cue becomes one slide per verse with auto-fitted text.
3. Fades between slides are smooth and never show a blank frame between two images.
4. A countdown overlay and the stage display turn red in overtime; a stage message shows on the
   stage display within 100 ms.
5. Lower third, logo bug, clock and ticker can be shown and hidden independently while the show
   runs.

## Demo checklist

- [ ] Add an MP4 and a WebM video and an MP3; play, pause, seek and restart from the operator view
      and from an operator phone; loop and "next cue at the end" behave as set.
- [ ] Paste lyrics into a text cue: one slide per verse, text fills the screen.
- [ ] Set the default transition to fade 400 ms; step through image, text and video cues.
- [ ] Add a lower third, clock and ticker; toggle them while the show runs.
- [ ] Start a 1-minute countdown and open the stage display: yellow in the last minute, red and
      counting on in overtime; send a stage message.
- [ ] Choose a custom logo image; press L.

## Results

- Rust: protocol 41, core 53, render 9, server 20 tests. Frontend: 13 unit tests, 7 Playwright
  E2E tests.
- Verified in the real host on Linux (WebKitGTK): a WebM video starts with its cue, plays in
  sync on the output and the muted program monitor, pauses/resumes from the transport, reports
  its duration and auto-advances at the end; a video that cannot load keeps the previous frame
  on the output.

## Known limitations and deviations

- **Codecs** depend on the platform webview (ADR 0008, `docs/user/media.md`); on Linux,
  `gstreamer1.0-libav` and `gstreamer1.0-gl` are needed for H.264 and smooth video.
- **Transitions:** cut and fade only; slide/dissolve variants are listed in `docs/ideas.md`.
- **Countdown to a time of day** uses the viewing device's time zone (host and phones are
  normally in the same place).
- **Text auto-fit** happens in each view, so a phone thumbnail and the output may wrap lines
  slightly differently at very different aspect ratios.
- The stage display window is a single extra window; multiple outputs come in phase 3.
