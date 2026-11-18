// A person as the console names them (RFC 0006 D13): their name when their
// identity provider gave one, else a short form of their subject, never the
// raw 64-character hash.

/** What to show, and the full subject for the tooltip. */
export function personLabel(
  subject: string,
  name: string | null | undefined,
  unnamed: (id: string) => string,
): { label: string; title: string } {
  const label =
    name && name.trim() !== ""
      ? name
      : unnamed(subject.length > 12 ? subject.slice(0, 6) : subject);
  return { label, title: subject };
}
