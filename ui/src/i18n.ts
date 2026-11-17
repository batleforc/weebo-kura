import { createI18n } from "vue-i18n";

import en from "./locales/en.json";
import fr from "./locales/fr.json";

/** French or English, from the browser; English otherwise. */
export function localeOf(languages: readonly string[]): "fr" | "en" {
  return languages.some((l) => l.toLowerCase().startsWith("fr")) ? "fr" : "en";
}

export const i18n = createI18n({
  legacy: false,
  locale:
    typeof navigator === "undefined"
      ? "en"
      : localeOf(navigator.languages ?? []),
  fallbackLocale: "en",
  messages: { en, fr },
});
