/**
 * The open vault's event socket (RFC 0005 §7.5), as the plugin holds it: one
 * socket, opened with a single-use ticket; a `change` or `pull` notice calls
 * `onNotice`. It never polls: a dropped socket reconnects with backoff from
 * 1 to 60 s, then reports a notice for what changed meanwhile.
 */

import { withDevice } from "@/api/client";
import { createTicket } from "@/api/generated/sdk.gen";

/** Device revoked, access revoked: a new ticket cannot help. */
const FINAL = new Set([4002, 4005]);
const MAX_DELAY = 60_000;
/** The server pings every 30 s (a JSON `ping` notice): silence for 2.5 of them is a dead socket. */
const SILENCE = 75_000;

export interface Watch {
  close(): void;
}

export function watchVault(vault: string, onNotice: () => void): Watch {
  let ws: WebSocket | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let watchdog: ReturnType<typeof setTimeout> | undefined;
  let delay = 1000;
  let dropped = false;
  let closed = false;

  const retry = () => {
    if (closed) return;
    dropped = true;
    timer = setTimeout(() => void connect(), delay);
    delay = Math.min(delay * 2, MAX_DELAY);
  };

  async function connect(): Promise<void> {
    let ticket: string;
    try {
      ticket = (
        await withDevice((headers) =>
          createTicket({ headers, path: { v: vault }, throwOnError: true }),
        )
      ).data.ticket;
    } catch {
      retry();
      return;
    }
    if (closed) return;
    const url = new URL(
      `/v1/vaults/${encodeURIComponent(vault)}/events`,
      window.location.href,
    );
    url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
    url.searchParams.set("ticket", ticket);
    const socket = new WebSocket(url);
    ws = socket;
    // Any message (the ping included) proves the socket alive; silence drops it.
    const alive = () => {
      clearTimeout(watchdog);
      watchdog = setTimeout(() => {
        if (ws !== socket) return;
        ws = null;
        socket.onclose = null;
        socket.close();
        retry();
      }, SILENCE);
    };
    socket.onopen = () => {
      alive();
      if (dropped) onNotice();
      dropped = false;
    };
    socket.onmessage = (e: MessageEvent) => {
      // Reset on a message, not on open: a socket closed at once keeps backing off.
      delay = 1000;
      alive();
      if (typeof e.data !== "string") return;
      try {
        const n: unknown = JSON.parse(e.data);
        if (
          typeof n === "object" &&
          n !== null &&
          "type" in n &&
          (n.type === "change" || n.type === "pull")
        )
          onNotice();
      } catch {
        // Not a notice.
      }
    };
    socket.onclose = (e: CloseEvent) => {
      if (ws !== socket) return;
      clearTimeout(watchdog);
      ws = null;
      if (!FINAL.has(e.code)) retry();
    };
  }

  void connect();
  return {
    close() {
      closed = true;
      clearTimeout(timer);
      clearTimeout(watchdog);
      ws?.close();
      ws = null;
    },
  };
}
