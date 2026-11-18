import { describe, expect, it } from "vitest";
import { personLabel } from "./people";

const unnamed = (id: string) => `unnamed · ${id}`;

describe("personLabel", () => {
  it("prefers the name, else shortens a long subject", () => {
    const sub =
      "f0f279fab2f67f36e4b6e84c9756a91dcb315bdf71190e7aa3ff6fc462ec7cf5";
    expect(personLabel(sub, "Bob", unnamed)).toEqual({
      label: "Bob",
      title: sub,
    });
    expect(personLabel(sub, null, unnamed).label).toBe("unnamed · f0f279");
    expect(personLabel(sub, "  ", unnamed).label).toBe("unnamed · f0f279");
    expect(personLabel("bob", undefined, unnamed).label).toBe("unnamed · bob");
  });
});
