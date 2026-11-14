import { describe, expect, it } from "vitest";
import { Controller, QUIET_MS } from "../src/controller";
import type { Situation } from "../src/status";
import { label, word } from "../src/status";
import { ProblemError, parseOutcome } from "../src/types";
import { ManualTimers } from "./fakes";

const report = { pushed: 0, pulled: 1, trashed: 0, conflicts: [], unsyncable: [], deferred: [], failed: [], refused: null, resynced: null };

function setup(
  run: (state: string, dryRun: boolean, allowDeletions: boolean) => Promise<unknown>,
  signedIn = true,
  confirm: (deletions: number, files: number) => Promise<boolean> = () => Promise.resolve(false),
) {
  const timers = new ManualTimers();
  const seen: Situation[] = [];
  const c = new Controller({ run, loadState: () => Promise.resolve(""), confirmDeletions: confirm, timers, onChange: (s) => { seen.push(s); }, signedIn: () => signedIn, role: () => "editor" });
  return { c, timers, seen };
}

describe("when to sync", () => {
  it("pushes 5 s after the last local change", () => {
    const t = setup(() => Promise.resolve({ state: {}, report }));
    t.c.touched();
    t.c.touched();
    expect(t.timers.delays()).toEqual([QUIET_MS]);
  });

  it("runs one sync at a time, queueing one more", async () => {
    let runs = 0;
    const t = setup((_s, dry) => { if (!dry) runs++; return Promise.resolve({ state: {}, report }); });
    const first = t.c.sync();
    const second = t.c.sync();
    expect(await second).toBeNull();
    expect((await first)?.pulled).toBe(1);
    expect(runs).toBe(2);
    expect(t.c.state.syncing).toBe(false);
  });

  it("reports refusals, outages and sign-out", async () => {
    const refused = setup(() => Promise.reject(new ProblemError(401, "device_revoked", null)));
    await refused.c.sync();
    expect(word(refused.c.state)).toBe("sign in");
    const offline = setup(() => Promise.reject(new Error("cannot reach")));
    await offline.c.sync();
    expect(word(offline.c.state)).toBe("offline");
    const out = setup(() => Promise.resolve({}), false);
    expect(await out.c.sync()).toBeNull();
    expect(label(out.c.state)).toBe("Kura: sign in");
    offline.c.setOnline(true);
    expect(offline.c.state.online).toBe(true);
  });
});

describe("a resync's deletions", () => {
  const guard = Object.assign(new ProblemError(0, "too_many_deletions", "61 of 400"), { deletions: 61, files: 400 });
  it("asks once, runs again when allowed, and waits for a manual sync after a no", async () => {
    const asked: number[] = [];
    let answer = false;
    const t = setup(
      (_s, dry, allow) => (dry || allow ? Promise.resolve({ state: {}, report: { ...report, trashed: allow ? 61 : 0 } }) : Promise.reject(guard)),
      true,
      (d) => { asked.push(d); return Promise.resolve(answer); },
    );
    await t.c.sync();
    expect(word(t.c.state)).toBe("confirm deletions");
    await t.c.sync();
    expect(asked).toEqual([61]);
    answer = true;
    expect((await t.c.sync(true))?.trashed).toBe(61);
    expect(asked).toEqual([61, 61]);
    expect(word(t.c.state)).toBe("in sync");
  });
});

describe("the words of §4.5", () => {
  const base: Situation = { signedIn: true, online: true, syncing: false, pending: 0, role: "editor", last: null, error: null };
  const p = (kind: string) => ({ status: 0, kind, detail: null });
  it("names every state", () => {
    expect(word(base)).toBe("in sync");
    expect(word({ ...base, syncing: true, pending: 14 })).toBe("syncing 14");
    expect(word({ ...base, syncing: true })).toBe("syncing");
    expect(word({ ...base, online: false })).toBe("offline");
    expect(word({ ...base, error: p("client_too_old") })).toBe("update required");
    expect(word({ ...base, error: p("history_rewound") })).toBe("resyncing");
    expect(word({ ...base, signedIn: false })).toBe("sign in");
    expect(word({ ...base, last: { ...report, refused: p("quota_exceeded") } })).toBe("quota full");
    expect(word({ ...base, role: "viewer", pending: 3 })).toBe("read-only (viewer)");
    expect(word({ ...base, last: { ...report, conflicts: ["a", "b"] } })).toBe("2 conflicts");
    expect(word({ ...base, last: { ...report, conflicts: ["a"] } })).toBe("1 conflict");
    expect(word({ ...base, last: { ...report, unsyncable: [["x", "y"]] } })).toBe("unsyncable");
  });
  it("checks what the core returns", () => {
    expect(() => parseOutcome(null)).toThrow();
    const o = parseOutcome({ state: { cursor: 1 }, report: { pushed: "x", conflicts: ["a", 1], unsyncable: [["p", "w"], ["bad"]], failed: 3, refused: { status: 403, kind: "insufficient_role" }, resynced: "cursor_expired" } });
    expect(o.state).toBe('{"cursor":1}');
    expect(o.report).toMatchObject({ pushed: 0, conflicts: ["a"], unsyncable: [["p", "w"]], failed: [], refused: { kind: "insufficient_role" }, resynced: "cursor_expired" });
  });
});
