/** OIDC discovery for the console's client (RFC 0003 §6.2). */

import { config } from "@/config";

export interface ProviderMetadata {
  issuer: string;
  authorization_endpoint: string;
  token_endpoint: string;
  revocation_endpoint?: string;
  code_challenge_methods_supported?: string[];
}

let cached: Promise<ProviderMetadata> | null = null;

/** Cached once per page load; a failure is not cached. */
export function discover(): Promise<ProviderMetadata> {
  if (!cached) {
    const attempt = load();
    cached = attempt;
    attempt.catch(() => {
      if (cached === attempt) cached = null;
    });
  }
  return cached;
}

const trim = (s: string) => s.replace(/\/+$/, "");

async function load(): Promise<ProviderMetadata> {
  const { issuer, discovery } = config();
  const url = `${trim(discovery)}/.well-known/openid-configuration`;
  const response = await fetch(url, {
    headers: { accept: "application/json" },
  });
  if (!response.ok) {
    throw new Error(
      `The identity provider did not answer discovery (${response.status}).`,
    );
  }
  const metadata = (await response.json()) as ProviderMetadata;
  if (trim(metadata.issuer ?? "") !== trim(issuer)) {
    throw new Error(
      `The discovery document names another issuer (${metadata.issuer}).`,
    );
  }
  if (!metadata.authorization_endpoint || !metadata.token_endpoint) {
    throw new Error(
      "The discovery document is missing an endpoint the console needs.",
    );
  }
  // The page talks to one identity provider origin, the one the server's
  // Content-Security-Policy allows in `connect-src`.
  const origin = new URL(discovery).origin;
  for (const endpoint of [
    metadata.authorization_endpoint,
    metadata.token_endpoint,
    metadata.revocation_endpoint,
  ]) {
    if (endpoint && new URL(endpoint).origin !== origin) {
      throw new Error(
        `The discovery document points outside ${origin} (${endpoint}).`,
      );
    }
  }
  const methods = metadata.code_challenge_methods_supported;
  if (methods && !methods.includes("S256")) {
    throw new Error("The identity provider does not support PKCE with S256.");
  }
  return metadata;
}

/** Tests only. */
export function forgetDiscovery(): void {
  cached = null;
}
