// SPDX-License-Identifier: GPL-3.0-or-later
// Minimal ICU MessageFormat i18n with a reactive locale (Svelte 5 runes).
import { IntlMessageFormat } from "intl-messageformat";
import de from "../locales/de.json";
import en from "../locales/en.json";

export type Messages = Record<string, string>;
export type MessageKey = keyof typeof en;
export type MessageValues = Record<string, string | number | boolean | Date>;

const catalogs: Record<string, Messages> = { en, de };
// Formatter cache; intentionally non-reactive.
// eslint-disable-next-line svelte/prefer-svelte-reactivity
const cache = new Map<string, IntlMessageFormat>();
const state = $state({ locale: "en" });

export const fallbackLocale = "en";

/** Register (or extend) a catalog for a locale. */
export function addMessages(locale: string, messages: Messages): void {
  catalogs[locale] = { ...catalogs[locale], ...messages };
  for (const k of cache.keys()) if (k.startsWith(`${locale}\u0000`)) cache.delete(k);
}

/** Languages with a catalog, each named in its own language (not translated). */
export const LANGUAGES: { code: string; name: string }[] = [
  { code: "en", name: "English" },
  { code: "de", name: "Deutsch" },
];

/**
 * The locale to use: `choice` if it has a catalog, otherwise the first of the browser's
 * preferred languages that has one (matching `de-AT` to `de`), otherwise English.
 */
export function preferredLocale(choice?: string | null, preferred?: readonly string[]): string {
  if (choice && catalogs[choice]) return choice;
  const langs = preferred ?? (typeof navigator === "undefined" ? [] : navigator.languages);
  for (const lang of langs ?? []) {
    const code = lang.toLowerCase();
    if (catalogs[code]) return code;
    const base = code.split("-")[0] ?? code;
    if (catalogs[base]) return base;
  }
  return fallbackLocale;
}

export function setLocale(locale: string): void {
  state.locale = catalogs[locale] ? locale : fallbackLocale;
  if (typeof document !== "undefined") document.documentElement.lang = state.locale;
}

export function getLocale(): string {
  return state.locale;
}

export function availableLocales(): string[] {
  return Object.keys(catalogs);
}

/** Translate `key`. Falls back to English, then to the key itself. */
export function t(key: MessageKey, values?: MessageValues): string {
  const locale = state.locale;
  const source = catalogs[locale]?.[key] ?? catalogs[fallbackLocale]?.[key];
  if (source === undefined) return key;
  const usedLocale = catalogs[locale]?.[key] !== undefined ? locale : fallbackLocale;
  const cacheKey = `${usedLocale}\u0000${key}`;
  let fmt = cache.get(cacheKey);
  if (!fmt) {
    fmt = new IntlMessageFormat(source, usedLocale);
    cache.set(cacheKey, fmt);
  }
  return String(fmt.format(values));
}
