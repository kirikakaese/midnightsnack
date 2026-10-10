# Translating midnightsnack

Every text in the operator window, the outputs, the stage display and the phone remote comes
from one catalog per language in `packages/ui/src/locales/`. English (`en.json`) is the source;
other languages translate its keys. Missing keys fall back to English, so a partial
translation already works.

## Add a language

1. Copy `en.json` to `<code>.json`, where `<code>` is the language's
   [BCP 47](https://www.rfc-editor.org/info/bcp47) code in lower case (`fr`, `pt-br`).
2. Translate the values. Keep the keys, and keep every `{placeholder}` exactly as it is.
3. Register the catalog in `packages/ui/src/i18n/index.svelte.ts`: import the file, add it to
   `catalogs`, and add `{ code, name }` to `LANGUAGES` with the language's name in that
   language (`Français`, not `French`).
4. Run `pnpm i18n:check`. It fails on keys that do not exist in `en.json` and reports how many
   keys are still untranslated.

Phones pick the first of their browser's languages that has a catalog (a `de-AT` browser gets
`de`); the host uses the language chosen in **Control → Language**, or the system's.

## Message format

Messages use [ICU MessageFormat](https://formatjs.github.io/docs/core-concepts/icu-syntax/):

- `{name}` inserts a value: `"upload.added": "{name} was added to the show."`
- Plurals: `"{n, plural, one {# slide} other {# slides}}"`. Use the plural categories your
  language has (`zero`, `one`, `two`, `few`, `many`, `other`; `=0` for an exact number); `#`
  is the number, formatted for the language.
- An apostrophe right before `{`, `}` or `#` starts quoted text in ICU; write `’` for
  apostrophes to be safe.

Numbers, dates, times and file sizes are formatted by the code with the language's
conventions, so catalogs only contain words.

## Terms

Some words are kept in English in the German translation because they are what people say in
event technology, and appear on control surfaces: **GO**, **Blackout**, **Freeze**, **Logo**,
**Cue**, **Overlay**, **Timer**, **Countdown**, **Relay**, **Hotspot**. Decide the same for
your language with what the people running shows use; be consistent.

Address the user the way your language's software usually does (the German catalog uses the
informal "du", like most current German software).

## For developers

- Never put user-facing text in code. Add the key to `en.json` and use `t("key")`; errors are
  codes (`ErrorCode` in `crates/protocol`) that the frontend shows as `t("error.<code>")`.
- Keys are grouped by area (`cue.`, `inspector.`, `relay.`, …). Dynamic keys built with a
  template (`` t(`role.${role}`) ``) must map to keys that exist; the type checker checks
  literal keys, and `pnpm i18n:check` fails on English keys no code refers to (literally or
  by such a prefix).
- When you change the meaning of an English text, change its key too, so translations of the
  old meaning are not shown for the new one.
- Format numbers and dates with `Intl` and `getLocale()`, never by hand.
