// One `.obsidian` run (RFC 0010 §5.2): three passes, one per space, each
// with its own state and only the paths whose home is that space, so the
// copies never mix. The vault's plugin code first, then the member's shared
// items, then this profile's own. A pass that fails leaves the others going.
import type { Core } from "./core";
import { ObsidianHost } from "./host";
import type { Adapter, HostOptions } from "./host";
import { PluginGuard, VaultUpdates, codeHashes, forget, pluginOf } from "./plugins";
import type { Consent, PluginRegistry } from "./plugins";
import type { Api, Shares, Space, VaultView } from "./remote";

export interface ConfigRun {
  core: Core;
  api: Api;
  adapter: Adapter;
  reg: PluginRegistry;
  vault: VaultView;
  /** `.obsidian`. */
  configDir: string;
  /** This plugin's folder, for the states and temporary files. */
  pluginDir: string;
  /** The same folder relative to `.obsidian` (`plugins/weebo-kura`): never synced. */
  own: string;
  consent: Consent;
  save: () => Promise<void>;
  /** The core's options of the notes run (device, vault, chunking, sizes). */
  options: Record<string, unknown>;
  now: () => number;
  random: () => Uint8Array;
  ask: {
    /** A plugin of the member's own configuration (decision 34). */
    plugin: (id: string) => Promise<boolean>;
    /** A plugin the vault's managers installed. */
    vaultPlugin: (id: string) => Promise<boolean | "never">;
    /** New code of a vault plugin agreed to before (decision 10). */
    update: (id: string) => Promise<"use" | "later" | "never">;
  };
}

export interface PassFailure {
  space: Space;
  error: unknown;
}

interface Pass {
  before?: (state: string) => Promise<void>;
  beforeWrite: (path: string) => Promise<void>;
  place?: (path: string) => string;
  remote?: { readOnly?: boolean; skip?: (path: string) => boolean };
  after: (hashes: Map<string, string>) => Promise<void>;
}

export async function syncConfig(r: ConfigRun): Promise<PassFailure[]> {
  const v = r.vault;
  let shares: Shares | null = null;
  try {
    shares = await r.api.shares(v.id);
  } catch {
    // shortcut: an older server or a restricted viewer runs the profile pass
    // only, with no vault plugin known; on a vault taking plugins its code
    // paths are then refused (422) for that run, never lost.
  }
  const shared = shares?.shared ?? v.configShared;
  const onVault = shares?.vaultPlugins ?? [];
  const own = r.own.toLowerCase();
  const never = (p: string): boolean => r.consent.never.includes(pluginOf(p)?.id ?? "");
  // The server's copy first on first contact (§4.2 "Changing a choice"); files only.
  const options = JSON.stringify({ ...r.options, folders: false, preferRemote: true });
  const failures: PassFailure[] = [];

  const pass = async (space: Space, stateName: string, home: string, p: Pass): Promise<void> => {
    const keep = (path: string): boolean =>
      (own === "" || !path.toLowerCase().startsWith(`${own}/`)) &&
      r.core.configHome(path, v.configSync, shared, onVault) === home &&
      !(home === "vault" && never(path));
    const o: HostOptions = {
      adapter: r.adapter,
      pluginDir: r.pluginDir,
      vault: v.id,
      isOpen: () => false,
      now: r.now,
      random: r.random,
      root: r.configDir,
      keep,
      stateName,
      beforeWrite: p.beforeWrite,
      ...(p.place === undefined ? {} : { place: p.place }),
    };
    const host = new ObsidianHost(o);
    try {
      // A path no longer of this space is forgotten here, never deleted there.
      const state = forget(await host.loadState(), keep);
      await p.before?.(state);
      await r.core.sync(host, r.api.remote(v.id, space, p.remote ?? {}), state, options);
      await p.after(codeHashes(await host.loadState()));
    } catch (error) {
      failures.push({ space, error });
    }
  };

  if (v.vaultPlugins && shares !== null) {
    const guard = new PluginGuard(r.reg, r.consent, r.save, r.ask.vaultPlugin);
    const updates = new VaultUpdates({ adapter: r.adapter, root: r.configDir, reg: r.reg, consent: r.consent, save: r.save, ask: r.ask.update });
    await pass("config-vault", "config-vault-state", "vault", {
      before: (state) => updates.load(state),
      beforeWrite: async (path) => {
        if (!(await updates.divert(path))) await guard.beforeWrite(path);
      },
      place: (path) => updates.place(path),
      // A member reads the vault's code; a local edit of it is never offered.
      remote: { readOnly: v.role !== "manager", skip: never },
      after: async (hashes) => {
        await updates.review(hashes);
        await guard.review(hashes);
      },
    });
  }
  const guard = new PluginGuard(r.reg, r.consent, r.save, r.ask.plugin);
  const mine: Pass = { beforeWrite: (path) => guard.beforeWrite(path), after: (hashes) => guard.review(hashes) };
  if (shares !== null) await pass("config-shared", "config-shared-state", "shared", mine);
  await pass("config", "config-state", "profile", mine);
  return failures;
}
