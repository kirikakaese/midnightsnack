// SPDX-License-Identifier: GPL-3.0-or-later
// Minimal ICU MessageFormat i18n with a reactive locale (Svelte 5 runes).
import { IntlMessageFormat } from "intl-messageformat";
import en from "../locales/en.json";

export type Messages = Record<string, string>;
export type MessageKey = keyof typeof en;
export type MessageValues = Record<string, string | number | boolean | Date>;

const catalogs: Record<string, Messages> = { en };
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

export function setLocale(locale: string): void {
  state.locale = catalogs[locale] ? locale : fallbackLocale;
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
