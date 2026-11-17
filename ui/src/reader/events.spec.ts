import { createPinia, setActivePinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/api/generated/sdk.gen", () => ({
  createTicket: vi.fn(async () => ({ data: { ticket: "t1", expiresIn: 30 } })),
  registerDevice: vi.fn(async () => ({ data: { id: "dev" } })),
}));

import * as sdk from "@/api/generated/sdk.gen";
import { watchVault } from "./events";

/** A WebSocket that the test drives. */
class FakeSocket {
  static all: FakeSocket[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((e: { data: unknown }) => void) | null = null;
  onclose: ((e: { code: number }) => void) | null = null;
  closed = false;
  constructor(readonly url: URL) {
    FakeSocket.all.push(this);
  }
  close() {
    this.closed = true;
  }
  send(data: unknown) {
    this.onmessage?.({ data });
  }
}

const last = () => FakeSocket.all.at(-1);

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
  FakeSocket.all = [];
  vi.useFakeTimers();
  vi.stubGlobal("WebSocket", FakeSocket);
  vi.clearAllMocks();
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("event socket", () => {
  it("opens one socket with a ticket, and calls back on change and pull only", async () => {
    const notice = vi.fn();
    const w = watchVault("v1", notice);
    await vi.runAllTicks();
    await vi.advanceTimersByTimeAsync(0);
    expect(FakeSocket.all).toHaveLength(1);
    const url = last()?.url;
    expect(url?.protocol).toBe("ws:");
    expect(url?.pathname).toBe("/v1/vaults/v1/events");
    expect(url?.searchParams.get("ticket")).toBe("t1");
    last()?.onopen?.();
    last()?.send(JSON.stringify({ type: "ping", headSeq: 3 }));
    last()?.send("not json");
    expect(notice).not.toHaveBeenCalled();
    last()?.send(JSON.stringify({ type: "change", seq: 4 }));
    last()?.send(JSON.stringify({ type: "pull" }));
    expect(notice).toHaveBeenCalledTimes(2);
    w.close();
    expect(last()?.closed).toBe(true);
    // Never polls: nothing more happens once closed.
    await vi.advanceTimersByTimeAsync(10 * 60_000);
    expect(FakeSocket.all).toHaveLength(1);
    expect(notice).toHaveBeenCalledTimes(2);
  });

  it("reconnects with a growing delay, catches up once open, and stops on revocation", async () => {
    const notice = vi.fn();
    const w = watchVault("v1", notice);
    await vi.advanceTimersByTimeAsync(0);
    last()?.onclose?.({ code: 1006 });
    await vi.advanceTimersByTimeAsync(999);
    expect(FakeSocket.all).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(FakeSocket.all).toHaveLength(2);
    last()?.onclose?.({ code: 1006 });
    await vi.advanceTimersByTimeAsync(1999);
    expect(FakeSocket.all).toHaveLength(2);
    await vi.advanceTimersByTimeAsync(1);
    expect(FakeSocket.all).toHaveLength(3);
    // What changed while it was down.
    last()?.onopen?.();
    expect(notice).toHaveBeenCalledOnce();
    // A failed ticket backs off too.
    vi.mocked(sdk.createTicket).mockRejectedValueOnce(new Error("down"));
    last()?.onclose?.({ code: 1006 });
    await vi.advanceTimersByTimeAsync(4000);
    expect(FakeSocket.all).toHaveLength(3);
    await vi.advanceTimersByTimeAsync(8000);
    expect(FakeSocket.all).toHaveLength(4);
    last()?.onclose?.({ code: 4005 });
    await vi.advanceTimersByTimeAsync(10 * 60_000);
    expect(FakeSocket.all).toHaveLength(4);
    w.close();
  });
});
