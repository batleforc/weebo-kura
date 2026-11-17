/**
 * What the server tells the page at `/app/config.json` (RFC 0003 §4.4): the
 * identity provider and the console's public client. Read once, at start.
 */

export interface ConsoleConfig {
  /** `oidc.issuer`: what the tokens name. */
  issuer: string;
  /** Where the discovery document is (Authentik's per-application URL). */
  discovery: string;
  clientId: string;
}

let loaded: ConsoleConfig | null = null;

export async function loadConfig(
  fetcher: typeof fetch = fetch,
): Promise<ConsoleConfig> {
  const response = await fetcher("/app/config.json", {
    headers: { accept: "application/json" },
  });
  if (!response.ok) {
    throw new Error(`/app/config.json answered ${response.status}`);
  }
  const body: unknown = await response.json();
  const read = (key: string): string => {
    const value =
      typeof body === "object" && body !== null
        ? Reflect.get(body, key)
        : undefined;
    if (typeof value !== "string" || value === "") {
      throw new Error(`/app/config.json has no ${key}`);
    }
    return value;
  };
  loaded = {
    issuer: read("issuer"),
    discovery: read("discovery"),
    clientId: read("clientId"),
  };
  return loaded;
}

/** The loaded configuration; the application mounts only after `loadConfig`. */
export function config(): ConsoleConfig {
  if (!loaded) {
    throw new Error("the console configuration is not loaded yet");
  }
  return loaded;
}

/** Tests only. */
export function setConfig(c: ConsoleConfig | null): void {
  loaded = c;
}
