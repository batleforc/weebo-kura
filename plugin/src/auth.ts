// Sign-in (RFC 0001 §6.2): the OAuth device authorization grant (RFC 8628)
// as a public client, the refresh token in Obsidian's SecretStorage under an
// id naming the server and the vault (decision 53), and one refresh in flight.
import { isRecord } from "./types";

export interface HttpRequest {
  url: string;
  method: "GET" | "POST";
  form?: Record<string, string>;
}
export interface HttpResponse {
  status: number;
  json: unknown;
}
/** Obsidian's `requestUrl` in the plugin, a fake in tests. */
export type Http = (req: HttpRequest) => Promise<HttpResponse>;
export type Sleep = (ms: number) => Promise<void>;

export type AuthFailure = "expired" | "denied" | "signed_out" | "unavailable";
export class AuthError extends Error {
  constructor(readonly reason: AuthFailure, message: string) {
    super(message);
  }
}

export interface Discovery {
  tokenEndpoint: string;
  deviceEndpoint: string;
}
export interface DeviceCode {
  deviceCode: string;
  userCode: string;
  verificationUri: string;
  verificationUriComplete: string | null;
  expiresIn: number;
  interval: number;
}
export interface Tokens {
  accessToken: string;
  refreshToken: string | null;
  /** Seconds. */
  expiresIn: number;
}

function str(v: unknown, field: string): string {
  if (isRecord(v) && typeof v[field] === "string") return v[field];
  throw new AuthError("unavailable", `the identity provider sent no ${field}`);
}
function num(v: unknown, field: string, fallback: number): number {
  return isRecord(v) && typeof v[field] === "number" ? v[field] : fallback;
}

async function call(http: Http, req: HttpRequest): Promise<HttpResponse> {
  try {
    return await http(req);
  } catch (e) {
    throw new AuthError("unavailable", `cannot reach the identity provider: ${String(e)}`);
  }
}

export async function discover(http: Http, issuer: string): Promise<Discovery> {
  const r = await call(http, { url: `${issuer.replace(/\/+$/, "")}/.well-known/openid-configuration`, method: "GET" });
  if (r.status !== 200) throw new AuthError("unavailable", `discovery answered ${String(r.status)}`);
  return { tokenEndpoint: str(r.json, "token_endpoint"), deviceEndpoint: str(r.json, "device_authorization_endpoint") };
}

export async function startDevice(http: Http, d: Discovery, clientId: string): Promise<DeviceCode> {
  const r = await call(http, {
    url: d.deviceEndpoint,
    method: "POST",
    form: { client_id: clientId, scope: "openid profile email offline_access" },
  });
  if (r.status !== 200) throw new AuthError("unavailable", `the device endpoint answered ${String(r.status)}`);
  const complete = isRecord(r.json) && typeof r.json.verification_uri_complete === "string" ? r.json.verification_uri_complete : null;
  return {
    deviceCode: str(r.json, "device_code"),
    userCode: str(r.json, "user_code"),
    verificationUri: str(r.json, "verification_uri"),
    verificationUriComplete: complete,
    expiresIn: num(r.json, "expires_in", 600),
    interval: num(r.json, "interval", 5),
  };
}

function tokens(json: unknown): Tokens {
  const refresh = isRecord(json) && typeof json.refresh_token === "string" ? json.refresh_token : null;
  return { accessToken: str(json, "access_token"), refreshToken: refresh, expiresIn: num(json, "expires_in", 300) };
}

function oauthError(json: unknown): string {
  return isRecord(json) && typeof json.error === "string" ? json.error : "";
}

/** Polls at the interval the provider asks for, until approval, denial or expiry. */
export async function waitForApproval(
  http: Http,
  d: Discovery,
  clientId: string,
  code: DeviceCode,
  sleep: Sleep,
  now: () => number,
  cancelled: () => boolean = () => false,
): Promise<Tokens> {
  let interval = Math.max(1, code.interval);
  const deadline = now() + code.expiresIn * 1000;
  for (;;) {
    await sleep(interval * 1000);
    if (cancelled()) throw new AuthError("denied", "sign-in cancelled");
    if (now() > deadline) throw new AuthError("expired", `the code was not approved within ${String(Math.round(code.expiresIn / 60))} minutes`);
    const r = await call(http, {
      url: d.tokenEndpoint,
      method: "POST",
      form: { grant_type: "urn:ietf:params:oauth:grant-type:device_code", device_code: code.deviceCode, client_id: clientId },
    });
    if (r.status === 200) return tokens(r.json);
    const e = oauthError(r.json);
    if (e === "authorization_pending") continue;
    if (e === "slow_down") {
      interval += 5;
      continue;
    }
    if (e === "expired_token") throw new AuthError("expired", "the code expired");
    throw new AuthError("denied", "the sign-in was refused");
  }
}

export async function refresh(http: Http, d: Discovery, clientId: string, refreshToken: string): Promise<Tokens> {
  const r = await call(http, {
    url: d.tokenEndpoint,
    method: "POST",
    form: { grant_type: "refresh_token", refresh_token: refreshToken, client_id: clientId },
  });
  if (r.status === 200) return tokens(r.json);
  // Only invalid_grant signs out; a network error or 5xx keeps the token (§6.2).
  if (oauthError(r.json) === "invalid_grant") throw new AuthError("signed_out", "signed out: sign in again");
  throw new AuthError("unavailable", `the token endpoint answered ${String(r.status)}`);
}

/** FNV-1a, hex: a short stable name for a server URL in a secret id. */
function fnv(s: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h.toString(16).padStart(8, "0");
}

/** SecretStorage ids are lowercase alphanumerics and dashes; this one names
 * the server and the vault, since the store may be shared across vaults (decision 53). */
export function secretId(server: string, vault: string): string {
  return `kura-${fnv(server.replace(/\/+$/, "").toLowerCase())}-${vault.toLowerCase().replace(/[^a-z0-9-]/g, "")}`;
}

/** Tokens only travel over https; http only to this machine, for local tests (review 4 L4). */
export function secureUrl(url: string): boolean {
  try {
    const u = new URL(url);
    return u.protocol === "https:" || (u.protocol === "http:" && ["localhost", "127.0.0.1", "[::1]"].includes(u.hostname));
  } catch {
    return false;
  }
}

/** `preferred_username`, for display only, never trusted. */
export function username(accessToken: string): string | null {
  const part = accessToken.split(".")[1];
  if (part === undefined) return null;
  try {
    const json: unknown = JSON.parse(atob(part.replace(/-/g, "+").replace(/_/g, "/")));
    return isRecord(json) && typeof json.preferred_username === "string" ? json.preferred_username : null;
  } catch {
    return null;
  }
}

export interface Secrets {
  getSecret(id: string): string | null;
  setSecret(id: string, secret: string): void;
}

/** Access tokens for API calls: cached until a minute before expiry, then
 * one refresh in flight, the rotated refresh token stored before use (§6.2). */
export class Session {
  private access: { token: string; until: number } | null = null;
  private inFlight: Promise<string> | null = null;

  constructor(
    private readonly http: Http,
    private readonly issuer: string,
    private readonly clientId: string,
    private readonly secrets: Secrets,
    private readonly id: string,
    private readonly now: () => number,
  ) {}

  signedIn(): boolean {
    return (this.secrets.getSecret(this.id) ?? "") !== "";
  }

  /** After a sign-in: keep the refresh token, use the access token now. */
  adopt(t: Tokens): void {
    if (t.refreshToken !== null) this.secrets.setSecret(this.id, t.refreshToken);
    this.access = { token: t.accessToken, until: this.now() + (t.expiresIn - 60) * 1000 };
  }

  signOut(): void {
    this.secrets.setSecret(this.id, "");
    this.access = null;
  }

  async token(): Promise<string> {
    if (this.access !== null && this.now() < this.access.until) return this.access.token;
    this.inFlight ??= this.renew().finally(() => {
      this.inFlight = null;
    });
    return this.inFlight;
  }

  private async renew(): Promise<string> {
    const stored = this.secrets.getSecret(this.id) ?? "";
    if (stored === "") throw new AuthError("signed_out", "not signed in");
    const d = await discover(this.http, this.issuer);
    try {
      const t = await refresh(this.http, d, this.clientId, stored);
      this.adopt(t);
      return t.accessToken;
    } catch (e) {
      if (e instanceof AuthError && e.reason === "signed_out") this.signOut();
      throw e;
    }
  }
}
