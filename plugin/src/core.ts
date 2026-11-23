// The WASM core, instantiated once from the bytes esbuild embeds in main.js.
import { configHome, configSynced, initSync, pathProblem, sync } from "./wasm/kura";
import type { KuraHost, KuraRemote } from "./wasm/kura";

export interface Core {
  sync(host: KuraHost, remote: KuraRemote, state: string, options: string): Promise<unknown>;
  pathProblem(path: string): string | undefined;
  /** A path under `.obsidian` syncs with these categories (§6.8). */
  configSynced(path: string, categories: string[]): boolean;
  /** Where a configuration path lives (RFC 0010 §4.2): `vault`, `shared`,
   * `profile`, or undefined when it does not sync. */
  configHome(path: string, categories: string[], shared: string[], vaultPlugins: string[]): string | undefined;
}

export function loadCore(bytes: Uint8Array): Core {
  initSync({ module: bytes });
  return {
    sync: async (host, remote, state, options): Promise<unknown> => {
      const out: unknown = await sync(host, remote, state, options);
      return out;
    },
    pathProblem: (path) => pathProblem(path),
    configSynced: (path, categories) => configSynced(path, categories),
    configHome: (path, categories, shared, vaultPlugins) => configHome(path, categories, shared, vaultPlugins),
  };
}
