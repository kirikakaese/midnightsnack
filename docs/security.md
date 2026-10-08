# Threat model

This document grows with the features that expose attack surface. The pairing, LAN transport
and relay sections are written in the phases that introduce them (1 and 5).

## Assets

- Control of what is shown on the audience screen.
- Files on the host machine (shows, media, anything readable by the app).
- Content of the show (may be confidential before the event).

## Trust boundaries

- The host machine and its operator are trusted.
- Everything reaching the host over the network is untrusted until paired and authorized.
