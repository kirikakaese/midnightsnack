# Phase 2 — Media & live content

Goal: everything beyond static slides that a typical event needs — video and audio, text and
lyrics, countdowns, smooth transitions, overlays and a proper stage display.

## Tasks

### Model and engine (`crates/core`)
- [ ] Media cues (video, audio): loop, start/end trim, volume, auto-advance at the end; shared
      playback timeline (play, pause, seek, restart) so every view shows the same position.
- [ ] Text cues: slides from plain text, lyrics mode splits verses on blank lines, otherwise
      `---` separates slides; themes (font, size or auto-fit, colors, alignment, background
      color or image) with a show default and per-cue override.
- [ ] Timer cues: countdown for a duration, countdown to a wall-clock time, count-up, clock;
      overtime color.
- [ ] Global countdown (for overlays, stage display and remotes) with start/pause/reset.
- [ ] Per-cue transition (cut or fade with duration) and a show default; auto-advance after a
      delay per slide.
- [ ] Overlays defined in the show (lower third, logo bug, clock, ticker, timer) with position
      and style; visibility toggled live.
- [ ] Custom logo screen image.
- [ ] Stage messages from the operator to stage displays.
- [ ] Permissions for all new actions; bundles carry new media (backgrounds, logos).

### Server
- [ ] Media file endpoint with HTTP range requests (video seeking) behind the media key.
- [ ] Auto-advance scheduler (slide delays and media end).
- [ ] Output windows report media duration and end of playback (local only).

### Frontends
- [ ] Output: video/audio playback synchronized to the host timeline, preloading the next media
      cue, transitions (GPU-friendly opacity), overlays layer, text and timer rendering with
      auto-fit; never shows controls, spinners or errors.
- [ ] Operator: add video/audio, text/lyrics and timer cues; cue inspector (transition,
      auto-advance, media options, text editor, theme); media transport and progress; countdown
      and stage message controls; overlay manager with quick toggles; logo image.
- [ ] Stage display: dedicated window on the host and a stage view on remotes — current/next,
      notes, big timers, countdown with overtime color, operator message.
- [ ] Remote: media progress and transport, countdown and overlays for operators.

### Docs and tests
- [ ] Codec matrix per OS webview (`docs/user/media.md`); media layer ADR (webview HTML5
      media behind an interface; libmpv later).
- [ ] Unit tests for engine additions, server tests for range requests and new permissions,
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
