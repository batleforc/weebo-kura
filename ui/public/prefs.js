// The stored preferences (system|light|dark, system|en|fr) resolved before
// first paint: a file, not an inline script, so `script-src 'self'` allows it.
// src/prefs.ts keeps them in step afterwards; the two must resolve alike.
(() => {
  const read = (key) => {
    try {
      return localStorage.getItem(key) || "system";
    } catch {
      return "system";
    }
  };
  const root = document.documentElement;
  const theme = read("kura:theme");
  root.dataset.theme =
    theme === "system"
      ? matchMedia("(prefers-color-scheme: light)").matches
        ? "light"
        : "dark"
      : theme;
  const locale = read("kura:locale");
  root.lang =
    locale === "system"
      ? (navigator.languages || []).some((l) => l.toLowerCase().startsWith("fr"))
        ? "fr"
        : "en"
      : locale;
})();
