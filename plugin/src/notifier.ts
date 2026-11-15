// The vault's notifications (§6.3): a socket opened with a ticket; `change`
// and `pull` notices ask for a pull. While it is down, a 60 s ±20% timer
// pulls instead, and it reconnects with decorrelated jitter from 1 to 60 s.

/** The browser's WebSocket in the plugin, a fake in tests. */
export interface SocketLike {
  onmessage: ((e: { data: unknown }) => void) | null;
  onclose: (() => void) | null;
  onopen: (() => void) | null;
  close(): void;
}

export interface Timers {
  set(fn: () => void, ms: number): number;
  clear(id: number): void;
}

export interface NotifierOptions {
  /** A fresh ticket and the socket URL it opens. */
  open: () => Promise<string>;
  socket: (url: string) => SocketLike;
  /** Pull now. */
  onNotice: () => void;
  onState: (up: boolean) => void;
  timers: Timers;
  /** In [0, 1). */
  random: () => number;
}

/** A notice the server sends every 30 s; twice that silence means down. */
export const SILENCE_MS = 75_000;
export const POLL_MS = 60_000;

export class Notifier {
  private ws: SocketLike | null = null;
  private stopped = true;
  private backoff = 1;
  private silence: number | null = null;
  private retry: number | null = null;
  private poll: number | null = null;

  constructor(private readonly o: NotifierOptions) {}

  start(): void {
    this.stopped = false;
    void this.connect();
  }

  stop(): void {
    this.stopped = true;
    for (const t of [this.silence, this.retry, this.poll]) if (t !== null) this.o.timers.clear(t);
    this.silence = this.retry = this.poll = null;
    this.ws?.close();
    this.ws = null;
  }

  private heard(): void {
    if (this.silence !== null) this.o.timers.clear(this.silence);
    this.silence = this.o.timers.set(() => {
      this.down();
    }, SILENCE_MS);
  }

  private isStopped(): boolean {
    return this.stopped;
  }

  private async connect(): Promise<void> {
    if (this.isStopped()) return;
    let url: string;
    try {
      url = await this.o.open();
    } catch {
      this.down();
      return;
    }
    if (this.isStopped()) return;
    const ws = this.o.socket(url);
    this.ws = ws;
    ws.onopen = () => {
      if (this.poll !== null) this.o.timers.clear(this.poll);
      this.poll = null;
      this.o.onState(true);
      this.heard();
      // What changed while the socket was down (review 2026-11-15 #7).
      this.o.onNotice();
    };
    ws.onmessage = (e) => {
      // A socket that opens then closes at once (4005, 4008) keeps backing off.
      this.backoff = 1;
      this.heard();
      if (typeof e.data !== "string") return;
      try {
        const n: unknown = JSON.parse(e.data);
        if (typeof n === "object" && n !== null && "type" in n && (n.type === "change" || n.type === "pull")) this.o.onNotice();
      } catch {
        // Not a notice.
      }
    };
    ws.onclose = () => {
      if (this.ws === ws) this.down();
    };
  }

  /** The socket is down: pull on a timer and reconnect later. */
  private down(): void {
    if (this.stopped) return;
    this.ws?.close();
    this.ws = null;
    this.o.onState(false);
    if (this.silence !== null) this.o.timers.clear(this.silence);
    this.silence = null;
    if (this.poll === null) this.schedulePoll();
    // Decorrelated jitter: between 1 s and three times the last delay, at most 60 s.
    this.backoff = Math.min(60, 1 + Math.floor(this.o.random() * Math.max(1, this.backoff * 3 - 1)));
    if (this.retry !== null) this.o.timers.clear(this.retry);
    this.retry = this.o.timers.set(() => {
      this.retry = null;
      void this.connect();
    }, this.backoff * 1000);
  }

  private schedulePoll(): void {
    const ms = POLL_MS * (0.8 + 0.4 * this.o.random());
    this.poll = this.o.timers.set(() => {
      this.poll = null;
      this.o.onNotice();
      if (this.ws === null && !this.stopped) this.schedulePoll();
    }, ms);
  }
}
