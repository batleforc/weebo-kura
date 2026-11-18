import { describe, expect, it } from "vitest";
import type { DeviceSync } from "@/api/generated/types.gen";
import { byDistance, standing } from "./sync";

const d = (name: string, extra: Partial<DeviceSync>): DeviceSync => ({
  device: name,
  name,
  profile: "desktop",
  revoked: false,
  resync: false,
  ...extra,
});

describe("standing", () => {
  it("reads each state the server can answer", () => {
    expect(standing(d("a", {}))).toEqual({ kind: "never" });
    expect(standing(d("a", { behind: 0 }))).toEqual({ kind: "current" });
    expect(standing(d("a", { behind: 3 }))).toEqual({
      kind: "behind",
      n: 3,
      more: false,
    });
    expect(standing(d("a", { behind: 1001 }))).toEqual({
      kind: "behind",
      n: 1000,
      more: true,
    });
    expect(standing(d("a", { resync: true }))).toEqual({ kind: "resync" });
  });
  it("sorts a resync first, then most behind, then by name", () => {
    const rows = [
      d("z", { behind: 0 }),
      d("b", { behind: 3 }),
      d("y", {}),
      d("a", { behind: 3 }),
      d("r", { resync: true }),
    ];
    expect(byDistance(rows).map((r) => r.name)).toEqual([
      "r",
      "a",
      "b",
      "z",
      "y",
    ]);
  });
});
