/** The plugin as the console offers it: its download, and the link that sets it up. */

import { loadedConfig } from "@/config";

export interface PluginBuild {
  available: boolean;
  version?: string;
  file?: string;
  bytes?: number;
}

/** `/app/plugin.json`, written by the build; "not available" on any failure. */
export async function pluginBuild(
  fetcher: typeof fetch = fetch,
): Promise<PluginBuild> {
  try {
    const r = await fetcher("/app/plugin.json", {
      headers: { accept: "application/json" },
    });
    const body: unknown = r.ok ? await r.json() : null;
    if (typeof body !== "object" || body === null) return { available: false };
    const get = (k: string): unknown => Reflect.get(body, k);
    return get("available") === true &&
      typeof get("version") === "string" &&
      typeof get("file") === "string" &&
      typeof get("bytes") === "number"
      ? {
          available: true,
          version: String(get("version")),
          file: String(get("file")),
          bytes: Number(get("bytes")),
        }
      : { available: false };
  } catch {
    return { available: false };
  }
}

/**
 * `obsidian://kura-setup?…`, which the plugin shows and applies once accepted;
 * `null` when the server does not say how the plugin signs in.
 */
export function setupLink(
  server: string,
  vault?: { id: string; name: string },
): string | null {
  const plugin = loadedConfig()?.plugin;
  if (!plugin) return null;
  const params: [string, string][] = [
    ["server", server],
    ["issuer", plugin.issuer],
    ["clientId", plugin.clientId],
  ];
  // Obsidian keeps `vault` for its own vault name: Kura's are prefixed.
  if (vault)
    params.push(["kuraVault", vault.id], ["kuraVaultName", vault.name]);
  return `obsidian://kura-setup?${params.map(([k, v]) => `${k}=${encodeURIComponent(v)}`).join("&")}`;
}

export type Device = "desktop" | "android" | "ios";

/** The device reading the page, to open on its own install steps. */
export function deviceOf(ua: string, touchPoints: number): Device {
  if (/Android/i.test(ua)) return "android";
  // iPadOS announces itself as a Mac, with touch.
  if (/iPhone|iPad|iPod/i.test(ua) || (/Macintosh/.test(ua) && touchPoints > 1))
    return "ios";
  return "desktop";
}
