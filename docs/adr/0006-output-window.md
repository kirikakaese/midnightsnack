# 0006. Output window strategy

- **Status:** accepted
- **Date:** 2026-10-08

## Context

The output must cover the projector, show nothing but content, and never misbehave in front of
an audience. OS fullscreen has side effects: macOS animates into a separate Space and can be
exited with a three-finger swipe; some Linux window managers move fullscreen windows to the
focused monitor.

## Decision

- The output is a borderless, always-on-top, non-focusable window positioned and sized to the
  chosen monitor (not OS fullscreen). It is hidden from the taskbar and hides the cursor.
- The output only renders state from the host. New slides are swapped in after they have fully
  loaded and decoded (`SlideImage`); on any error or disconnect the last frame stays up.
- Slides are pre-rendered at the output's native pixel size; the window reports its size to the
  server, which prefetches the current, previous and next three slides.
- The chosen display is remembered by name and the output reopens there on launch if present.
- Screen sleep is inhibited while the output is open (`keepawake`).

## Consequences

- On Wayland, applications cannot position their own windows; compositors may place the output
  on the wrong display. Workaround: move it with the compositor, or run the host under XWayland.
  Revisit with layer-shell or portal APIs in phase 3 (display hotplug work).
- Hotplug detection and multiple outputs are deferred to phase 3.
