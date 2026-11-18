/** The rows whose fields contain the query, any case; an empty query keeps them all. */
export function matching<T>(
  rows: T[],
  query: string,
  fields: (row: T) => string[],
): T[] {
  const q = query.trim().toLowerCase();
  if (q === "") return rows;
  return rows.filter((r) => fields(r).some((f) => f.toLowerCase().includes(q)));
}
