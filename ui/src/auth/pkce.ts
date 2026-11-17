/**
 * Authorization Code + PKCE (RFC 7636), pure functions on Web Crypto, after
 * weebo-notify-v2's `web/src/auth/pkce.ts` (RFC 0003 §6.2, R30). No OIDC library.
 */

/** Base64url without padding, the encoding OAuth asks for. */
export function base64url(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary)
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}

/** `length` bytes of CSPRNG output, base64url encoded. */
export function randomToken(length = 32): string {
  const bytes = new Uint8Array(length);
  crypto.getRandomValues(bytes);
  return base64url(bytes);
}

/** S256 only: `plain` would put the verifier in URLs and logs. */
export async function challengeOf(verifier: string): Promise<string> {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(verifier),
  );
  return base64url(new Uint8Array(digest));
}

/** What a login attempt has to remember while the browser is away at the IdP. */
export interface PendingAuthorization {
  /** The PKCE secret, redeemed with the code. */
  verifier: string;
  /** Anti-CSRF value echoed back by the provider (RFC 6749 §10.12). */
  state: string;
  /** Where to go once the session is open, so a permalink survives the login. */
  returnTo: string;
}

/** Everything a fresh login needs, generated in one place. */
export async function beginAuthorization(returnTo: string): Promise<{
  pending: PendingAuthorization;
  challenge: string;
}> {
  const pending: PendingAuthorization = {
    // 32 bytes → 43 base64url characters, inside the RFC 7636 range of 43–128.
    verifier: randomToken(32),
    state: randomToken(16),
    returnTo,
  };
  return { pending, challenge: await challengeOf(pending.verifier) };
}

/**
 * Where a sign-in may land: a path inside the application, nothing else
 * (RFC 0003 T9). Anything that could leave the origin becomes `/`.
 */
export function inAppTarget(target: string | null | undefined): string {
  if (
    !target?.startsWith("/") ||
    target.startsWith("//") ||
    target.includes("\\")
  ) {
    return "/";
  }
  return target;
}
