# User guide

- [Getting started](getting-started.md) — install, build a show, put it on the projector
- [Video, audio, text, timers and overlays](media.md) — including the codec matrix
- [Several screens](outputs.md) — outputs, cue targets, displays, test patterns
- [Presentations, web pages and screen capture](sources.md) — PowerPoint/Keynote, OpenSlides projector
- [OpenSlides](openslides.md) — agenda, motions and lists of speakers shown natively
- [Phones and tablets as remotes](remotes.md) — pairing, roles, stage view
- [Network, hotspot, HTTPS and relay](connectivity.md) — when phones are not on the same network
- [Hosting a relay](relay.md) — Docker, reverse proxies, settings
- [Keyboard and presentation clickers](keyboard.md)
- [MIDI, OSC, HTTP API, Stream Deck and a second computer](control.md)
- [Troubleshooting and FAQ](troubleshooting.md)

## Installing unsigned builds

Releases are not signed with an Apple or Microsoft certificate yet, so your operating system
warns you the first time:

- **macOS** (also when installed with Homebrew): open DECK once; when macOS blocks it, open
  **System Settings → Privacy & Security** and click **Open Anyway**.
- **Windows:** in the SmartScreen dialog choose **More info → Run anyway**.
- **Linux:** make the AppImage executable (`chmod +x`) or install the `.deb`.
