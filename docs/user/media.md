# Video, audio, text and timers

## Video and audio

Add files with **+ Files**. Supported extensions: video `mp4 m4v mov webm mkv ogv`, audio
`mp3 m4a aac wav ogg oga opus flac`. Whether a file actually plays depends on the codecs your
operating system's web engine supports:

| Format                      | macOS (WKWebView) | Windows (WebView2) | Linux (WebKitGTK + GStreamer)        |
| --------------------------- | ----------------- | ------------------ | ------------------------------------ |
| MP4 · H.264 + AAC           | ✅                | ✅                 | ✅ with `gstreamer1.0-libav`          |
| MP4 · H.265/HEVC            | ✅                | ⚠️ needs the HEVC extension and hardware support | ⚠️ with `gstreamer1.0-libav` |
| WebM · VP8/VP9 + Opus/Vorbis | ✅ (macOS 12+; VP8 on recent versions) | ✅ | ✅                         |
| WebM/MP4 · AV1              | ⚠️ Apple silicon with recent macOS | ✅ with hardware or AV1 extension | ⚠️ with `gstreamer1.0-plugins-bad` |
| MOV · ProRes                | ✅                | ❌                 | ❌                                    |
| MKV                         | ❌                | ⚠️ depends on codecs | ✅                                  |
| MP3, AAC/M4A, WAV           | ✅                | ✅                 | ✅ (`gstreamer1.0-plugins-good`, AAC needs `-libav`) |
| Ogg Vorbis, Opus, FLAC      | ⚠️ Opus/FLAC yes, Vorbis on recent versions | ✅ | ✅                       |

**Recommendation:** export videos as **MP4 with H.264 video and AAC audio** — it plays
everywhere. On Linux install:

```sh
sudo apt install gstreamer1.0-libav gstreamer1.0-plugins-good gstreamer1.0-gl
```

If a file cannot be played, the output simply keeps showing the previous slide; nothing broken
is ever shown to the audience.

### Playback options (Cue tab)

- **Loop**, **Go to the next cue when it ends**
- **Start / End** trim (m:ss)
- **Volume**
- **Transition** and **auto-advance** like any other cue

The video starts when its cue goes live. Use the transport under the program monitor (or a phone
with the operator role) to pause, restart and seek. Freezing the output on a video keeps it
playing while you prepare the next cue.

## Text and lyrics

**+ Add… → Text** creates a text cue. Edit it in the **Cue** tab:

- **Lyrics** mode: every verse (separated by an empty line) becomes one slide.
- Otherwise put a line containing only `---` between slides.

Text is fitted to the screen automatically, or set a fixed size. The look (font, colors,
alignment, background image) comes from the show's default (**Show** tab) unless the cue has its
own.

## Timers

**+ Add… → Timer** adds a timer slide: count down a duration (starts when the cue goes live), count down
to a time of day, count up, or show the clock. A countdown turns to the overtime color (and
counts on with a `+`) once it passes zero.

Separately, the **Live** tab has a global **countdown** for talks: start, pause and reset it,
show it as an overlay, and presenters see it large on the stage display — yellow in the last
minute, red in overtime.

## Overlays

Overlays sit on top of every cue: **lower third** (name and title), **logo bug** (corner image),
**clock**, **ticker** (scrolling text) and **countdown**. Create them in the **Live** tab, choose
position, colors and size, and toggle them with one click — also from an operator phone.

## Transitions and the logo screen

The **Show** tab sets the default transition (cut, or fade with a duration). A cue can override
it. Blackout and the logo screen fade with the default transition. The logo screen can show your
own image instead of the built-in logo.

## Stage display

**Outputs → Stage display** opens a window for a screen facing the presenter: current and next
slide, notes, show and slide timers, the clock, the countdown, and messages you send from the
**Live** tab. Phones get the same view via **Stage view** (stage viewers always see it).
