# 0008. Media playback in the webview

- **Status:** accepted
- **Date:** 2026-10-08

## Context

Video and audio cues must start with the cue, survive freeze, and stay in sync between the
output, the operator's monitor and phones. Native players (libmpv) give the best codec coverage
but add a large, platform-specific dependency and a second rendering path for the output.

## Decision

- Playback uses HTML5 `<video>`/`<audio>` in the output webview. Tauri's webview (wry) allows
  autoplay with sound on all platforms.
- The **host owns the timeline.** `LiveState.media` holds a stopwatch of the file position for
  the cue on the output (media follows the *output*, so freezing on a video keeps it playing).
  Play, pause, seek and restart are actions like any other.
- Every view derives the position from that timeline. Small drift (< 1 s) is corrected by
  adjusting `playbackRate` by up to ±8 % (inaudible); larger drift seeks, at most every 2 s.
- Only output windows report back to the host — the file's duration (`media_loaded`) and the end
  of playback (`media_ended`); both are local-only actions. With a known duration the engine
  also schedules the end on its own, so auto-advance does not depend on the report.
- Media files are served by the embedded server with HTTP range requests (seeking) behind the
  per-connection media key. Phones never decode media; they show progress only.
- All media rendering lives in `packages/ui/src/stage/MediaContent.svelte`, behind the content
  descriptor produced by `describe()`. A native backend (e.g. libmpv rendering into the output
  window) can later replace the element for the `output` mode without touching the protocol.

## Consequences

- Codec support is whatever the platform webview supports; see `docs/user/media.md`.
  H.264/AAC in MP4 and VP9/Opus in WebM are the safe choices.
- On Linux, WebKitGTK uses GStreamer: users need `gstreamer1.0-libav` for H.264 and
  `gstreamer1.0-gl` for efficient video rendering.
