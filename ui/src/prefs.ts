/**
 * The two reader preferences, theme and locale (batlehub-design, The
 * Stored-Preference Rule): the choice is stored, never what it resolved to,
 * so "system" follows the device when it changes. public/prefs.js applies the
 * same resolution before first paint.
 */

import { ref } from "vue";

import { i18n, localeOf } from "@/i18n";

export type ThemePref = "system" | "light" | "dark";
export type LocalePref = "system" | "en" | "fr";

/** The stored choice, or "system" when there is none or storage is refused. */
function read<T extends string>(
  key: string,
  allowed: readonly T[],
): T | "system" {
  try {
    const v = localStorage.getItem(key);
    return allowed.find((a) => a === v) ?? "system";
  } catch {
    return "system";
  }
}

function store(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Private mode: the choice lasts for this page only.
  }
}

const lightQuery = () => matchMedia("(prefers-color-scheme: light)");

export const themePref = ref<ThemePref>(
  read("kura:theme", ["light", "dark"] as const),
);
export const localePref = ref<LocalePref>(
  read("kura:locale", ["en", "fr"] as const),
);

/** The theme in force, for what cannot read `data-theme` itself (the note frame). */
export const theme = ref<"light" | "dark">("dark");

export function resolvedTheme(): "light" | "dark" {
  if (themePref.value !== "system") return themePref.value;
  return lightQuery().matches ? "light" : "dark";
}

export function resolvedLocale(): "en" | "fr" {
  if (localePref.value !== "system") return localePref.value;
  return localeOf(navigator.languages ?? []);
}

function apply(): void {
  theme.value = resolvedTheme();
  document.documentElement.dataset.theme = theme.value;
  const locale = resolvedLocale();
  i18n.global.locale.value = locale;
  document.documentElement.lang = locale;
}

export function setTheme(pref: ThemePref): void {
  themePref.value = pref;
  store("kura:theme", pref);
  apply();
}

export function setLocale(pref: LocalePref): void {
  localePref.value = pref;
  store("kura:locale", pref);
  apply();
}

/** Once, at start: apply, then follow the device while a preference is "system". */
export function installPrefs(): void {
  apply();
  lightQuery().addEventListener("change", apply);
  window.addEventListener("languagechange", apply);
}
