/** Dates for the reader, through `Intl` (RFC 0005 §11). */

const UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ["day", 86_400],
  ["hour", 3_600],
  ["minute", 60],
];

/** "2 hours ago", "now": an RFC 3339 time against `now`. */
export function ago(iso: string, locale: string, now = Date.now()): string {
  const s = (Date.parse(iso) - now) / 1000;
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  for (const [unit, n] of UNITS)
    if (Math.abs(s) >= n) return rtf.format(Math.round(s / n), unit);
  return rtf.format(0, "second");
}

/** A calendar date in the reader's locale. */
export function day(iso: string, locale: string): string {
  return new Intl.DateTimeFormat(locale, { dateStyle: "medium" }).format(
    new Date(iso),
  );
}
