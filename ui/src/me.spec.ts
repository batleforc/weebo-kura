import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as sdk from "@/api/generated/sdk.gen";
import { useMe } from "./me";

vi.mock("@/api/generated/sdk.gen", () => ({ me: vi.fn() }));

beforeEach(() => {
  setActivePinia(createPinia());
  vi.mocked(sdk.me).mockReset();
});

describe("useMe", () => {
  it("asks once when several callers load together, then not again", async () => {
    vi.mocked(sdk.me).mockResolvedValue({
      data: { subject: "s", platformAdmin: false, tenants: [] },
    } as never);
    const store = useMe();
    await Promise.all([store.load(), store.load(), store.load()]);
    await store.load();
    expect(sdk.me).toHaveBeenCalledTimes(1);
  });

  it("asks again after a failure", async () => {
    vi.mocked(sdk.me).mockRejectedValueOnce(new Error("429"));
    vi.mocked(sdk.me).mockResolvedValueOnce({
      data: { subject: "s", platformAdmin: false, tenants: [] },
    } as never);
    const store = useMe();
    await expect(store.load()).rejects.toThrow("429");
    expect((await store.load()).subject).toBe("s");
  });
});
