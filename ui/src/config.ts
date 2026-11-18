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
  /** What the "Open in Obsidian" link gives the plugin (`ui.plugin`); absent from an older server. */
  plugin?: { clientId: string; issuer: string };
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
  const plugin: unknown =
    typeof body === "object" && body !== null
      ? Reflect.get(body, "plugin")
      : undefined;
  const pluginField = (k: string): string | undefined => {
    const v: unknown =
      typeof plugin === "object" && plugin !== null
        ? Reflect.get(plugin, k)
        : undefined;
    return typeof v === "string" && v !== "" ? v : undefined;
  };
  const pluginClient = pluginField("clientId");
  const pluginIssuer = pluginField("issuer");
  loaded = {
    issuer: read("issuer"),
    discovery: read("discovery"),
    clientId: read("clientId"),
    ...(pluginClient && pluginIssuer
      ? { plugin: { clientId: pluginClient, issuer: pluginIssuer } }
      : {}),
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

/** The configuration when loaded, else `null`: for what can do without it. */
export function loadedConfig(): ConsoleConfig | null {
  return loaded;
}

/** Tests only. */
export function setConfig(c: ConsoleConfig | null): void {
  loaded = c;
}
