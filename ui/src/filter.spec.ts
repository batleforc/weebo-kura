import { describe, expect, it } from "vitest";
import { matching } from "./filter";

describe("matching", () => {
  const rows = [{ n: "Backup-CI" }, { n: "laptop" }];
  it("keeps every row for a blank query", () => {
    expect(matching(rows, "  ", (r) => [r.n])).toEqual(rows);
  });
  it("matches any case, on any field", () => {
    expect(matching(rows, "ci", (r) => [r.n])).toEqual([{ n: "Backup-CI" }]);
    expect(matching(rows, "zzz", (r) => [r.n])).toEqual([]);
  });
});
