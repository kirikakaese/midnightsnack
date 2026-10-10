# Connecting remotes: network, hotspot, HTTPS and relay

Remotes reach the laptop in one of these ways. All of them are set up in the **Connect** tab;
the QR code above the PIN can point at any of them (**QR code for**).

| Way                | When                                                             | Needs                    |
| ------------------ | ---------------------------------------------------------------- | ------------------------ |
| **Local network**  | Phones and laptop on the same Wi-Fi or wired network (default)   | Nothing                  |
| **Hotspot**        | No usable network, or a guest Wi-Fi that isolates devices        | Wi-Fi in the laptop      |
| **HTTPS**          | You want the local connection encrypted                          | Accepting a warning once |
| **Relay**          | Phones on another network or on mobile data                      | A relay server you run   |

## Local network

The **Network** list shows every address of the laptop with the interface it belongs to. The QR
code uses the first one; pick another under **QR code for** if phones are on a different
network (for example the wired network of a venue). The list updates by itself when you join
another network or start a hotspot.

If a phone cannot open the page, it is probably on another network, or the network blocks
devices from talking to each other (common on guest and hotel Wi-Fi). Use the hotspot or the
relay instead.

## Hotspot

The laptop becomes its own Wi-Fi network; phones join it and then scan the QR code. This works
without any internet connection. DECK recognizes the hotspot's address and puts it
first in the QR code selector.

- **Linux (NetworkManager):** enter a network name and a password (8 or more characters) under
  **Hotspot** and click **Start hotspot**. **Stop hotspot** ends it. The laptop's Wi-Fi is used
  for the hotspot while it runs, so the laptop itself is offline unless it has a wired
  connection.
- **Windows 10/11:** click **Open hotspot settings** (Settings → Network & internet → Mobile
  hotspot), choose *Share over Wi-Fi*, set the name and password with **Edit**, and turn it on.
  The address is usually `192.168.137.1`. Depending on the Windows version, the laptop may need
  some other network connection to share before Mobile hotspot can be turned on.
- **macOS:** System Settings → General → **Sharing** → ⓘ next to *Internet Sharing*: share a
  connection with **Wi-Fi**, set the name and password under *Wi-Fi Options*, then turn
  Internet Sharing on. macOS requires another connection (e.g. Ethernet or iPhone USB) to share.

**Show Wi-Fi QR code** shows a code phones scan with their camera to join the hotspot in one
step (enter the name and password you set in the system settings on Windows and macOS). Then
they scan the pairing QR code as usual.

## HTTPS on the local network

**Also serve HTTPS** encrypts the connection between phones and the laptop. DECK
creates its own certificate (shown as a **fingerprint**) and serves HTTPS on port 4749 next to
the normal address. Because no public authority signed the certificate, browsers show a warning
the first time each phone opens the page:

1. Choose **HTTPS** under **QR code for** and scan it.
2. On the warning page, open the details and compare the certificate's SHA-256 fingerprint with
   the one in the Connect tab. If it matches, continue (*Advanced → Proceed* in Chrome, *Show
   Details → visit this website* in Safari).

**New certificate** replaces it (phones then show the warning again). The plain address keeps
working for devices where the warning is not acceptable. Certificates are valid for a little
over two years and renewed automatically.

## Relay

For phones that cannot reach the laptop directly, the laptop can connect to a **relay** on the
internet, and phones connect to the relay. Everything between phone and laptop is end-to-end
encrypted: the relay only forwards data it cannot read. DECK has no relay of its own —
use one that you, your organization or someone you trust runs ([how to host one](relay.md)).

1. Under **Relay**, enter the relay's address (e.g. `https://relay.example.org`) and, if its
   operator gave you one, the access token. Click **Connect**.
2. When it says **Connected**, choose **Relay** under **QR code for** and let phones scan it.
   Pairing works as usual (PIN and approval); requests that came through the relay are labeled.

Through the relay, phones see slides, notes and timers and can control the show, point, draw
and send files. Video, audio and screen capture are not previewed on the phone (it shows a
placeholder); they still play on the projector.

**Reset relay identity** creates new keys; devices joined through the relay must scan the QR
code again. Use it if a relay link was shared with people who should not have it.

### Automatic fallback

Phones paired on the local network learn the laptop's relay address. If the local network is
lost for about 8 seconds (for example the phone left the Wi-Fi), the phone shows **Switching to
the internet relay in 5 s…** and moves to the relay by itself — tap **Stay** to keep trying the
local network. On the relay, **Use local network** in the header goes back.

## Devices

The **Devices** list shows for each paired device how it is connected (this computer, local
network with its address, HTTPS, relay) and its latency, or when it was last seen. Click a name
to rename it. **Forget offline devices** removes all paired devices that are not connected right
now (API keys stay).
