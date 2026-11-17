/**
 * The console's session (RFC 0003 §4.2, §6.2): the only code that touches
 * storage. The access token lives in memory only; the refresh token in
 * `sessionStorage`, or in `localStorage` when the user ticked "Stay signed in
 * on this device" (decision 17). No cookie, no server-side session.
 */

import { defineStore } from "pinia";
import { computed, ref } from "vue";

import { config } from "@/config";
import { discover } from "./discovery";
import {
  beginAuthorization,
  inAppTarget,
  type PendingAuthorization,
} from "./pkce";

const PENDING_KEY = "kura:pending-auth";
const REFRESH_KEY = "kura:refresh-token";
const DEVICE_KEY = "kura:device";
const SCOPES = "openid profile offline_access";
/** Refresh this long before expiry, so no request rides a dead token. */
const MARGIN_MS = 60_000;

interface Pending extends PendingAuthorization {
  stay: boolean;
}

interface TokenResponse {
  access_token: string;
  refresh_token?: string;
  expires_in?: number;
}

/** The identity provider answered, and said no: only this ends a session. */
export class TokenRefused extends Error {
  constructor(
    readonly status: number,
    readonly error: string,
  ) {
    super(
      `The identity provider refused the token request (${status} ${error}).`,
    );
  }
}

/** Leaving for the identity provider; replaced in tests (jsdom cannot navigate). */
export const navigation = {
  assign(url: string): void {
    window.location.assign(url);
  },
};

export function redirectUri(): string {
  return `${window.location.origin}/app/callback`;
}

/** The box's default: off in a browser tab, on in the installed application. */
export function stayByDefault(): boolean {
  return (
    typeof window.matchMedia === "function" &&
    window.matchMedia("(display-mode: standalone)").matches
  );
}

/** Where the refresh token is, if anywhere: the storage the box chose. */
function stored(): { token: string; stay: boolean } | null {
  const kept = localStorage.getItem(REFRESH_KEY);
  if (kept) return { token: kept, stay: true };
  const tab = sessionStorage.getItem(REFRESH_KEY);
  return tab ? { token: tab, stay: false } : null;
}

function store(token: string, stay: boolean): void {
  (stay ? localStorage : sessionStorage).setItem(REFRESH_KEY, token);
  (stay ? sessionStorage : localStorage).removeItem(REFRESH_KEY);
}

/** One refresh across every tab of this origin; without it, rotation signs out the loser. */
function exclusive<T>(run: () => Promise<T>): Promise<T> {
  if (typeof navigator !== "undefined" && navigator.locks) {
    return navigator.locks.request("kura:refresh", run);
  }
  return run();
}

async function redeem(
  endpoint: string,
  body: URLSearchParams,
): Promise<TokenResponse> {
  const response = await fetch(endpoint, {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body,
  });
  if (!response.ok) {
    const problem = (await response.json().catch(() => ({}))) as {
      error?: string;
    };
    throw new TokenRefused(response.status, problem.error ?? "unknown");
  }
  return (await response.json()) as TokenResponse;
}

export const useSession = defineStore("session", () => {
  const accessToken = ref<string | null>(null);
  const expiresAt = ref(0);
  /** The session ended without the user signing out: shown with the revoke hint (decision 26). */
  const endedUnexpectedly = ref(false);
  let refreshing: Promise<string | null> | null = null;

  const authenticated = computed(() => accessToken.value !== null);

  function remember(tokens: TokenResponse, stay: boolean): void {
    // Stored before it is used: rotation already spent the previous one.
    // No `refresh_token` in the answer keeps the one held (decision 28).
    if (tokens.refresh_token) store(tokens.refresh_token, stay);
    accessToken.value = tokens.access_token;
    expiresAt.value = Date.now() + (tokens.expires_in ?? 300) * 1000;
  }

  function forget(): void {
    accessToken.value = null;
    expiresAt.value = 0;
    localStorage.removeItem(REFRESH_KEY);
    sessionStorage.removeItem(REFRESH_KEY);
    sessionStorage.removeItem(PENDING_KEY);
  }

  /** Leaves for the identity provider; comes back to `/app/callback`. */
  async function login(returnTo: string, stay: boolean): Promise<void> {
    const metadata = await discover();
    const { pending, challenge } = await beginAuthorization(
      inAppTarget(returnTo),
    );
    const remembered: Pending = { ...pending, stay };
    sessionStorage.setItem(PENDING_KEY, JSON.stringify(remembered));
    const url = new URL(metadata.authorization_endpoint);
    url.searchParams.set("response_type", "code");
    url.searchParams.set("client_id", config().clientId);
    url.searchParams.set("redirect_uri", redirectUri());
    url.searchParams.set("scope", SCOPES);
    url.searchParams.set("state", pending.state);
    // Always S256: Authentik would also issue a code without a challenge
    // (decision 27), so PKCE protects only the flows started here.
    url.searchParams.set("code_challenge", challenge);
    url.searchParams.set("code_challenge_method", "S256");
    navigation.assign(url.toString());
  }

  /** Redeems the code of `/app/callback`; returns the in-app path to go to. */
  async function completeLogin(params: URLSearchParams): Promise<string> {
    const raw = sessionStorage.getItem(PENDING_KEY);
    if (!raw) throw new Error("No sign-in was started from this tab.");
    const pending = JSON.parse(raw) as Pending;
    // Before anything else is believed, `error` included (T9).
    if (params.get("state") !== pending.state) {
      throw new Error(
        "The sign-in answer does not match the sign-in started here.",
      );
    }
    sessionStorage.removeItem(PENDING_KEY);
    const error = params.get("error");
    if (error) throw new Error(params.get("error_description") ?? error);
    const code = params.get("code");
    if (!code) throw new Error("The identity provider sent no code.");
    const metadata = await discover();
    const tokens = await redeem(
      metadata.token_endpoint,
      new URLSearchParams({
        grant_type: "authorization_code",
        code,
        redirect_uri: redirectUri(),
        client_id: config().clientId,
        code_verifier: pending.verifier,
      }),
    );
    remember(tokens, pending.stay);
    endedUnexpectedly.value = false;
    return inAppTarget(pending.returnTo);
  }

  /** A fresh access token, or `null`; `endedUnexpectedly` says whether the session is over. */
  function refresh(): Promise<string | null> {
    if (refreshing) return refreshing;
    refreshing = exclusive(async () => {
      // Read inside the lock: another tab may have rotated it meanwhile.
      const held = stored();
      if (!held) {
        accessToken.value = null;
        return null;
      }
      try {
        const metadata = await discover();
        const tokens = await redeem(
          metadata.token_endpoint,
          new URLSearchParams({
            grant_type: "refresh_token",
            refresh_token: held.token,
            client_id: config().clientId,
          }),
        );
        remember(tokens, held.stay);
        return tokens.access_token;
      } catch (error) {
        // Only `invalid_grant` ends it; a network failure keeps the token.
        if (error instanceof TokenRefused && error.error === "invalid_grant") {
          forget();
          endedUnexpectedly.value = true;
        }
        return null;
      }
    }).finally(() => {
      refreshing = null;
    });
    return refreshing;
  }

  /** The token for the next request, renewed first when about to expire. */
  async function bearer(): Promise<string | null> {
    if (accessToken.value && Date.now() < expiresAt.value - MARGIN_MS) {
      return accessToken.value;
    }
    return stored() ? refresh() : null;
  }

  /** At start: signed in again from the kept refresh token, if there is one. */
  async function restore(): Promise<boolean> {
    return (await bearer()) !== null;
  }

  function device(): string | null {
    return localStorage.getItem(DEVICE_KEY);
  }

  function setDevice(id: string | null): void {
    if (id) localStorage.setItem(DEVICE_KEY, id);
    else localStorage.removeItem(DEVICE_KEY);
  }

  /**
   * Signing out (§4.2, decisions 24 and 25): the storage is cleared, the web
   * device is signed out at Kura, and Authentik is asked to revoke the refresh
   * token. Its answer cannot be read (no CORS header), so it is not awaited.
   */
  async function signOut(
    signOutDevice: (device: string) => Promise<unknown>,
  ): Promise<void> {
    const held = stored();
    const d = device();
    if (d && accessToken.value) {
      await signOutDevice(d).catch(() => undefined);
    }
    setDevice(null);
    forget();
    endedUnexpectedly.value = false;
    const metadata = await discover().catch(() => null);
    if (held && metadata?.revocation_endpoint) {
      void fetch(metadata.revocation_endpoint, {
        method: "POST",
        mode: "no-cors",
        keepalive: true,
        body: new URLSearchParams({
          client_id: config().clientId,
          token: held.token,
        }),
      }).catch(() => undefined);
    }
  }

  return {
    accessToken,
    authenticated,
    endedUnexpectedly,
    bearer,
    completeLogin,
    device,
    login,
    refresh,
    restore,
    setDevice,
    signOut,
  };
});
