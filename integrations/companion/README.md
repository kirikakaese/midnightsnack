# Companion module for DECK

Controls [DECK](../../README.md) from [Bitfocus Companion](https://bitfocus.io/companion)
(Stream Deck and other surfaces): actions, feedbacks and variables over the host's WebSocket API.
Setup and the list of actions, feedbacks and variables are in
[docs/user/control.md](../../docs/user/control.md#bitfocus-companion-stream-deck).

```sh
pnpm --filter companion-module-midnightsnack build   # dist/ for Companion's developer modules
pnpm --filter companion-module-midnightsnack test    # unit tests (+ devserver test if built)
```
