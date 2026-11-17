/**
 * The generated client wired to the session: every request gets the access
 * token and, once registered, this browser's device (RFC 0003 §4.2). A `401`
 * gets one retry after a forced refresh.
 */

import type { AxiosError, InternalAxiosRequestConfig } from "axios";

import { useSession } from "@/auth/session";
import { client } from "./generated/client.gen";
import { registerDevice, signOutDevice } from "./generated/sdk.gen";
import { problemType } from "./problem";

type Retryable = InternalAxiosRequestConfig & { _retried?: boolean };

let installed = false;

export function installAuth(): void {
  if (installed) return;
  installed = true;
  const session = useSession();
  client.setConfig({
    ...client.getConfig(),
    auth: () => session.bearer().then((token) => token ?? undefined),
  });
  client.instance.interceptors.response.use(
    (response) => response,
    async (error: AxiosError) => {
      const request = error.config as Retryable | undefined;
      // `invalid_token` only: a refused device is not cured by a new token.
      if (
        error.response?.status !== 401 ||
        problemType(error) !== "invalid_token" ||
        !request ||
        request._retried
      ) {
        return Promise.reject(error);
      }
      request._retried = true;
      const token = await session.refresh();
      if (!token) return Promise.reject(error);
      request.headers.set("Authorization", `Bearer ${token}`);
      return client.instance.request(request);
    },
  );
}

/** A short, human name for this browser: "Firefox on Linux". */
export function browserName(ua: string = navigator.userAgent): string {
  const browser =
    [
      ["Edg/", "Edge"],
      ["OPR/", "Opera"],
      ["Firefox/", "Firefox"],
      ["Chrome/", "Chrome"],
      ["Safari/", "Safari"],
    ].find(([mark]) => mark !== undefined && ua.includes(mark))?.[1] ??
    "Browser";
  const system =
    [
      ["Android", "Android"],
      ["iPhone", "iOS"],
      ["iPad", "iOS"],
      ["Windows", "Windows"],
      ["Mac OS", "macOS"],
      ["Linux", "Linux"],
    ].find(([mark]) => mark !== undefined && ua.includes(mark))?.[1] ??
    "this system";
  return `${browser} on ${system}`;
}

/**
 * This browser's device id, registered at first use. The server hands back
 * the device this Authentik session already has (one per session, T12).
 */
export function ensureDevice(): Promise<string> {
  const session = useSession();
  const known = session.device();
  if (known) return Promise.resolve(known);
  // One registration at a time: the shell and the page ask together at start.
  registering ??= registerDevice({
    body: { name: browserName(), profile: "web" },
    throwOnError: true,
  })
    .then(({ data }) => {
      session.setDevice(data.id);
      return data.id;
    })
    .finally(() => {
      registering = null;
    });
  return registering;
}

let registering: Promise<string> | null = null;

/**
 * Runs a call that needs a device; a device the server no longer knows
 * (signed out elsewhere, purged after 30 days) is registered again, once.
 */
export async function withDevice<T>(
  call: (headers: { "X-Kura-Device": string }) => Promise<T>,
): Promise<T> {
  const device = await ensureDevice();
  try {
    return await call({ "X-Kura-Device": device });
  } catch (error) {
    if (problemType(error) !== "device_unknown") throw error;
    useSession().setDevice(null);
    return call({ "X-Kura-Device": await ensureDevice() });
  }
}

export async function signOut(): Promise<void> {
  await useSession().signOut((d) =>
    signOutDevice({ path: { d }, throwOnError: true }),
  );
}

export { client };
