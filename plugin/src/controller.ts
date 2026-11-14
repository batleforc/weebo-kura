// When the plugin syncs (§6.3): on start, on resume, on a notice, and 5 s
// after the last local change; one run at a time, a request during a run
// queues one more. The state it reports drives the status words of §4.5.
import type { Problem, Report } from "./types";
import { deletionsToConfirm, isProblem, parseOutcome } from "./types";
import type { Situation } from "./status";

/** Edits coalesce this long before a push, so autosave does not exhaust
 * `maxRevisionsPerEntry` (§6.3). */
export const QUIET_MS = 5_000;

export interface ControllerOptions {
  /** One run of the core: resolves to its `{ state, report }`. */
  run: (state: string, dryRun: boolean, allowDeletions: boolean) => Promise<unknown>;
  /** A full resync would move `deletions` of `files` local files to the
   * trash: asked once per manual sync, never on every notice (§6.3). */
  confirmDeletions: (deletions: number, files: number) => Promise<boolean>;
  loadState: () => Promise<string>;
  timers: { set(fn: () => void, ms: number): number; clear(id: number): void };
  onChange: (s: Situation) => void;
  signedIn: () => boolean;
  role: () => string | null;
}

export class Controller {
  private running = false;
  private again = false;
  private quiet: number | null = null;
  /** The user said no to a resync's deletions: not asked again until a manual sync. */
  private declined = false;
  private situation: Situation;

  constructor(private readonly o: ControllerOptions) {
    this.situation = { signedIn: o.signedIn(), online: true, syncing: false, pending: 0, role: o.role(), last: null, error: null };
  }

  private queued(): boolean {
    return this.again;
  }

  get state(): Situation {
    return this.situation;
  }

  private set(patch: Partial<Situation>): void {
    this.situation = { ...this.situation, ...patch, signedIn: this.o.signedIn(), role: this.o.role() };
    this.o.onChange(this.situation);
  }

  setOnline(online: boolean): void {
    this.set({ online });
  }

  /** A file changed locally: push once edits have been quiet for 5 s. */
  touched(): void {
    if (this.quiet !== null) this.o.timers.clear(this.quiet);
    this.quiet = this.o.timers.set(() => {
      this.quiet = null;
      void this.sync();
    }, QUIET_MS);
  }

  /** Counts local changes without sending anything (the "syncing N" word). */
  async pending(): Promise<number> {
    try {
      const out = parseOutcome(await this.o.run(await this.o.loadState(), true, false));
      return out.report.pushed;
    } catch {
      return 0;
    }
  }

  /** `manual`: the user asked (ribbon, command), so a pending deletion
   * question may be asked again. */
  async sync(manual = false): Promise<Report | null> {
    if (!this.o.signedIn()) {
      this.set({ error: null });
      return null;
    }
    if (this.running) {
      this.again = true;
      return null;
    }
    this.running = true;
    let last: Report | null = null;
    try {
      do {
        this.again = false;
        this.set({ syncing: true, pending: await this.pending() });
        let allow = false;
        for (;;) {
          try {
            const out = parseOutcome(await this.o.run(await this.o.loadState(), false, allow));
            last = out.report;
            this.set({ syncing: false, last, error: null, online: true, pending: out.report.refused === null ? 0 : out.report.pushed });
          } catch (e) {
            const ask = deletionsToConfirm(e);
            if (ask !== null && !allow && (manual || !this.declined)) {
              allow = await this.o.confirmDeletions(ask.deletions, ask.files);
              this.declined = !allow;
              if (allow) continue;
            }
            const error: Problem | null = isProblem(e) ? e : null;
            this.set({ syncing: false, error, online: error !== null });
          }
          break;
        }
      } while (this.queued());
    } finally {
      this.running = false;
    }
    return last;
  }
}
