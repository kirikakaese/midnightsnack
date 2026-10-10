# Troubleshooting and FAQ

## Phones

**The phone cannot open the page after scanning the QR code.**
The phone must reach the laptop: same network, no device isolation. Guest, hotel and many
company Wi-Fi networks stop devices from talking to each other. Try another address under
**QR code for** in the Connect tab, start the laptop's [hotspot](connectivity.md#hotspot), or
use a [relay](connectivity.md#relay). A firewall on the laptop must allow incoming connections
for DECK (port 4747, and 4749 for HTTPS); Windows and macOS ask on first start.

**"Wrong PIN." or "Too many attempts."**
After several wrong PINs from a device (or many overall) pairing pauses for a minute. Type the
PIN shown in the Connect tab right now; it changes after **Disconnect all remotes**.

**"This link is incomplete" / "This link does not match the host any more."**
The relay link is missing its key, or the host reset its relay identity. Scan the current QR
code again.

**The phone keeps reconnecting.**
The phone screen locked or the browser was in the background; it reconnects on its own. The
remote keeps the screen on while it is open where the browser allows it.

**The phone says "This remote does not match the host version."**
The host was updated. Reload the page.

**How do I unpair a phone?**
Connect tab → the device → **Remove device**. **Disconnect all remotes** forgets every phone
and second computer and changes the PIN; API keys stay (revoke them in the Control tab).

## Outputs

**The output opened on the wrong screen.**
Outputs tab → pick the display for the output → **Open output** again. DECK remembers
the display by name and reopens the output there when it is connected.

**The projector was unplugged during the show.**
The output window is hidden (it never jumps onto your own screen) and the Outputs tab says which
display is missing; the show keeps running. When the display is back, the output returns to it
on its own. See [outputs.md](outputs.md#when-a-display-is-unplugged).

**Video does not play (Linux).**
Install the GStreamer plugins for the format, usually `gstreamer1.0-libav` and
`gstreamer1.0-plugins-good`; see the [codec table](media.md#video-and-audio). The `.deb`
installs the basic ones and recommends the rest; the AppImage brings its own.

**Screen capture shows nothing (macOS).**
Allow screen recording for DECK in System Settings → Privacy & Security → Screen
Recording, then restart midnightsnack.

## Files

**PowerPoint or Keynote files cannot be added.**
Presentations are converted to PDF with LibreOffice (Keynote files with Keynote, on a Mac).
Install LibreOffice, or export the presentation as PDF and add that.

**Double-clicking a `.msnack` file does not open midnightsnack.**
The installers register the file type (on Linux the `.deb` installs it system-wide; the
AppImage does not register file types, use **Open…** there or an AppImage integration tool).
If DECK is already running, the show opens in it, after asking about unsaved changes.

**A show opened on another computer is missing media.**
Shows saved with **Save linked…** only point at the media files. Use **Save** or
**Save as…**, which put the media into the show file.

**"This is not a valid DECK show."**
The file is damaged, from a newer version of DECK, or refers to files of the wrong
type (a show cannot make DECK serve arbitrary files to phones).

**Where does DECK keep its data?**
Settings, paired devices, the autosaved show and accepted uploads are in the app's data
folder, readable only by your user account:

| System  | Folder                                                          |
| ------- | --------------------------------------------------------------- |
| Linux   | `~/.local/share/io.github.kirikakaese.deck` (settings of this computer in `~/.config/…`) |
| macOS   | `~/Library/Application Support/io.github.kirikakaese.deck` |
| Windows | `%APPDATA%\io.github.kirikakaese.deck`                 |

Deleting the folder resets DECK: all phones must pair again.

## Language

**How do I change the language?**
Control tab → **Language**. Phones follow their own browser language; the language can also be
chosen on the phone's pairing page. To add a language, see the
[translation guide](../dev/translating.md).
