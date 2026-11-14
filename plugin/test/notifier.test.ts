import { describe, expect, it } from "vitest";
import { Notifier, POLL_MS, SILENCE_MS } from "../src/notifier";
import type { SocketLike } from "../src/notifier";
import { ManualTimers } from "./fakes";

class FakeSocket implements SocketLike {
  onmessage: ((e: { data: unknown }) => void) | null = null;
  onclose: (() => void) | null = null;
  onopen: (() => void) | null = null;
  closed = false;
  close(): void {
    this.closed = true;
  }
}

function setup(open: () => Promise<string> = () => Promise.resolve("wss://x")) {
  const timers = new ManualTimers();
  const sockets: FakeSocket[] = [];
  let notices = 0;
  const states: boolean[] = [];
  const n = new Notifier({
    open,
    socket: () => { const s = new FakeSocket(); sockets.push(s); return s; },
    onNotice: () => { notices++; },
    onState: (up) => { states.push(up); },
    timers,
    random: () => 0.5,
  });
  return { n, timers, sockets, states, notices: () => notices };
}

const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

describe("notifications", () => {
  it("pulls on change and pull notices, not on pings", async () => {
    const t = setup();
    t.n.start();
    await flush();
    const s = t.sockets[0];
    expect(s).toBeDefined();
    s?.onopen?.();
    expect(t.states).toEqual([true]);
    s?.onmessage?.({ data: JSON.stringify({ type: "change", seq: 3 }) });
    s?.onmessage?.({ data: JSON.stringify({ type: "pull" }) });
    s?.onmessage?.({ data: JSON.stringify({ type: "ping", headSeq: 3 }) });
    s?.onmessage?.({ data: "not json" });
    s?.onmessage?.({ data: new Uint8Array() });
    expect(t.notices()).toBe(2);
    expect(t.timers.delays()).toEqual([SILENCE_MS]);
    t.n.stop();
    expect(s?.closed).toBe(true);
    expect(t.timers.pending.size).toBe(0);
  });

  it("when down, polls every 60 s ±20% and reconnects with backoff", async () => {
    const t = setup();
    t.n.start();
    await flush();
    t.sockets[0]?.onopen?.();
    t.sockets[0]?.onclose?.();
    expect(t.states).toEqual([true, false]);
    const delays = t.timers.delays().sort((a, b) => a - b);
    expect(delays[0]).toBe(2000);
    expect(delays[1]).toBe(POLL_MS);
    t.timers.fireAll();
    await flush();
    expect(t.notices()).toBe(1);
    expect(t.sockets).toHaveLength(2);
    t.n.stop();
  });

  it("treats silence and a failed ticket as down", async () => {
    let fail = true;
    const t = setup(() => (fail ? Promise.reject(new Error("offline")) : Promise.resolve("wss://x")));
    t.n.start();
    await flush();
    expect(t.states).toEqual([false]);
    fail = false;
    t.timers.fireAll();
    await flush();
    t.sockets[0]?.onopen?.();
    t.timers.fireAll();
    expect(t.states).toEqual([false, true, false]);
    t.n.stop();
  });
});
