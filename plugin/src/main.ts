// Kura Sync (RFC 0001 §6.8): Obsidian glue only. Sign-in, sync timing,
// notifications, the vault host and the status words live in tested modules.
import { MarkdownView, Modal, Notice, Platform, Plugin, requestUrl } from "obsidian";
import type { App } from "obsidian";
import wasmBytes from "./wasm/kura_bg.wasm";
import { isRecord } from "./types";
import { AuthError, Session, discover, secretId, secureUrl, startDevice, waitForApproval } from "./auth";
import type { DeviceCode, Http } from "./auth";
import { Controller } from "./controller";
import { loadCore } from "./core";
import type { Core } from "./core";
import { ObsidianHost } from "./host";
import { Notifier } from "./notifier";
import type { SocketLike } from "./notifier";
import { Api } from "./remote";
import type { VaultView } from "./remote";
import { KuraSettingTab } from "./settings";
import type { Situation } from "./status";
import { label } from "./status";
import { PluginGuard, codeHashes, emptyConsent, forget, readConsent } from "./plugins";
import type { Consent, PluginRegistry } from "./plugins";

export interface Settings {
  server: string;
  issuer: string;
  clientId: string;
  vault: string;
  vaultName: string;
  device: string;
  deviceName: string;
  /** Per device (§6.8): files above it are listed as unsyncable here. */
  maxFileMiB: number;
  /** Synced plugins agreed to or refused on this device (decision 34). */
  consent: Consent;
}

const DEFAULTS: Settings = { server: "", issuer: "", clientId: "kura-plugin", vault: "", vaultName: "", device: "", deviceName: "", maxFileMiB: 200, consent: emptyConsent() };

/** Stored settings, field by field: `data.json` is not trusted blindly. */
function readSettings(data: unknown): Settings {
  const d = isRecord(data) ? data : {};
  const text = (k: keyof Settings, fallback: string): string => {
    const v = d[k];
    return typeof v === "string" ? v : fallback;
  };
  const size = d.maxFileMiB;
  return {
    server: text("server", DEFAULTS.server),
    issuer: text("issuer", DEFAULTS.issuer),
    clientId: text("clientId", DEFAULTS.clientId),
    vault: text("vault", DEFAULTS.vault),
    vaultName: text("vaultName", DEFAULTS.vaultName),
    device: text("device", DEFAULTS.device),
    deviceName: text("deviceName", DEFAULTS.deviceName),
    maxFileMiB: typeof size === "number" && size > 0 ? size : DEFAULTS.maxFileMiB,
    consent: readConsent(d.consent),
  };
}

const http: Http = async (req) => {
  const body = req.form === undefined ? undefined : new URLSearchParams(req.form).toString();
  const r = await requestUrl({
    url: req.url,
    method: req.method,
    throw: false,
    ...(body === undefined ? {} : { body, contentType: "application/x-www-form-urlencoded" }),
  });
  let json: unknown = null;
  try {
    json = r.json;
  } catch {
    // Not JSON.
  }
  return { status: r.status, json };
};

const sleep = (ms: number): Promise<void> => new Promise((resolve) => window.setTimeout(resolve, ms));
const timers = { set: (fn: () => void, ms: number) => window.setTimeout(fn, ms), clear: (id: number) => { window.clearTimeout(id); } };
const random16 = (): Uint8Array => crypto.getRandomValues(new Uint8Array(16));

class CodeModal extends Modal {
  cancelled = false;
  constructor(app: App, private readonly code: DeviceCode) {
    super(app);
  }
  override onOpen(): void {
    this.setTitle("Sign in to Kura");
    const open = this.code.verificationUriComplete ?? this.code.verificationUri;
    this.contentEl.createEl("p", { text: `1. Open ${this.code.verificationUri}` });
    this.contentEl.createEl("button", { text: "Open" }).onclick = () => window.open(open);
    this.contentEl.createEl("p", { text: "2. Enter this code:" });
    this.contentEl.createEl("p", { text: this.code.userCode, cls: "kura-code" });
    this.contentEl.createEl("button", { text: "Copy" }).onclick = () => void navigator.clipboard.writeText(this.code.userCode);
    this.contentEl.createEl("p", { text: "Waiting for approval..." });
  }
  override onClose(): void {
    this.cancelled = true;
  }
}

/** A full resync would trash many files: the user decides (§6.3). */
/** Obsidian's plugin switchboard (`app.plugins`, not in the public API),
 * read defensively: if its shape changes, configuration sync never enables
 * anything. */
function registry(app: App): PluginRegistry | null {
  const plugins: unknown = Reflect.get(app, "plugins");
  if (!isRecord(plugins)) return null;
  const enabled = plugins.enabledPlugins;
  const on = plugins.enablePluginAndSave;
  const off = plugins.disablePluginAndSave;
  if (!(enabled instanceof Set) || typeof on !== "function" || typeof off !== "function") return null;
  const adapter = app.vault.adapter;
  // What Obsidian loads at its next start, besides what runs now: a plugin
  // listed there but not loaded is enabled too (phase 8 review M2).
  const listFile = `${app.vault.configDir}/community-plugins.json`;
  const listed = async (): Promise<string[]> => {
    try {
      const v: unknown = JSON.parse(await adapter.read(listFile));
      return Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : [];
    } catch {
      return [];
    }
  };
  // Ids compare without regard to case, as the guard folds them.
  const same = (a: unknown, id: string): boolean => typeof a === "string" && a.toLowerCase() === id;
  const actual = (id: string, list: string[]): string =>
    [...enabled].find((x): x is string => same(x, id)) ?? list.find((x) => same(x, id)) ?? id;
  return {
    isEnabled: async (id) => [...enabled].some((x) => same(x, id)) || (await listed()).some((x) => same(x, id)),
    enable: async (id) => {
      await Promise.resolve(Reflect.apply(on, plugins, [actual(id, await listed())]));
    },
    disable: async (id) => {
      const list = await listed();
      await Promise.resolve(Reflect.apply(off, plugins, [actual(id, list)]));
      if (list.some((x) => same(x, id))) await adapter.write(listFile, JSON.stringify(list.filter((x) => !same(x, id))));
    },
  };
}

/** The §4.5 mock: a synced plugin, or a new version of one, runs only on [Enable]. */
class PluginModal extends Modal {
  private answer: (yes: boolean) => void = () => undefined;
  constructor(app: App, private readonly id: string) {
    super(app);
  }
  ask(): Promise<boolean> {
    return new Promise((resolve) => {
      this.answer = resolve;
      this.open();
    });
  }
  override onOpen(): void {
    this.setTitle("Kura Sync");
    const manifests: unknown = Reflect.get(Reflect.get(this.app, "plugins") ?? {}, "manifests");
    const m: unknown = isRecord(manifests) ? manifests[this.id] : null;
    const name = isRecord(m) && typeof m.name === "string" ? m.name : this.id;
    const version = isRecord(m) && typeof m.version === "string" ? ` ${m.version}` : "";
    this.contentEl.createEl("p", { text: `Plugin "${name}"${version} was synced from another of your devices.` });
    this.contentEl.createEl("p", { text: "It is installed but disabled on this device." });
    this.contentEl.createEl("p", { text: "Plugins run code with access to this vault." });
    this.contentEl.createEl("button", { text: "Keep disabled" }).onclick = () => {
      this.close();
    };
    this.contentEl.createEl("button", { text: "Enable", cls: "mod-cta" }).onclick = () => {
      this.answer(true);
      this.answer = () => undefined;
      this.close();
    };
  }
  override onClose(): void {
    this.answer(false);
  }
}

class ConfirmModal extends Modal {
  private answer: (yes: boolean) => void = () => undefined;
  constructor(app: App, private readonly deletions: number, private readonly files: number) {
    super(app);
  }
  ask(): Promise<boolean> {
    return new Promise((resolve) => {
      this.answer = resolve;
      this.open();
    });
  }
  override onOpen(): void {
    this.setTitle("Kura: files deleted elsewhere");
    const share = this.files === 0 ? 100 : Math.round((this.deletions * 100) / this.files);
    this.contentEl.createEl("p", {
      text: `This device was away longer than the vault keeps deletions. ${String(this.deletions)} local files (${String(share)}%) were deleted on other devices. Move them to the trash?`,
    });
    this.contentEl.createEl("button", { text: "Move to trash", cls: "mod-warning" }).onclick = () => {
      this.answer(true);
      this.answer = () => undefined;
      this.close();
    };
    this.contentEl.createEl("button", { text: "Not now" }).onclick = () => {
      this.close();
    };
  }
  override onClose(): void {
    this.answer(false);
  }
}

export default class KuraPlugin extends Plugin {
  config: Settings = { ...DEFAULTS };
  latest: Situation | null = null;
  private resyncAnnounced = false;
  vaults: VaultView[] = [];
  private core: Core | null = null;
  private session: Session | null = null;
  private api: Api | null = null;
  private controller: Controller | null = null;
  private notifier: Notifier | null = null;
  private statusEl: HTMLElement | null = null;
  private ribbonEl: HTMLElement | null = null;
  private tab: KuraSettingTab | null = null;

  override async onload(): Promise<void> {
    this.config = readSettings(await this.loadData());
    this.core = loadCore(wasmBytes);
    if (!Platform.isMobile) this.statusEl = this.addStatusBarItem();
    // Mobile has no plugin status bar: the ribbon label carries the state (§4.5).
    this.ribbonEl = this.addRibbonIcon("refresh-cw", "Kura", () => {
      void this.controller?.sync(true);
    });
    this.tab = new KuraSettingTab(this.app, this);
    this.addSettingTab(this.tab);
    this.addCommand({ id: "sync-now", name: "Sync now", callback: () => void this.controller?.sync(true) });
    this.addCommand({ id: "sign-in", name: "Sign in", callback: () => void this.signIn() });
    const touched = (): void => this.controller?.touched();
    this.registerEvent(this.app.vault.on("create", touched));
    this.registerEvent(this.app.vault.on("modify", touched));
    this.registerEvent(this.app.vault.on("delete", touched));
    this.registerEvent(this.app.vault.on("rename", touched));
    // Resume: pull when the app comes back (§6.3).
    // The configuration is scanned at most every 5 minutes, and on resume (§6.8).
    this.registerInterval(window.setInterval(() => void this.controller?.sync(), 5 * 60 * 1000));
    this.registerDomEvent(document, "visibilitychange", () => {
      if (document.visibilityState === "visible") void this.controller?.sync();
    });
    this.app.workspace.onLayoutReady(() => void this.start());
  }

  override onunload(): void {
    this.notifier?.stop();
  }

  async saveSettings(): Promise<void> {
    await this.saveData(this.config);
  }

  secretsAvailable(): boolean {
    return "secretStorage" in this.app;
  }

  username(): string | null {
    return null;
  }

  private sessionFor(): Session | null {
    const s = this.config;
    if (!this.secretsAvailable() || s.server === "" || s.issuer === "" || s.vault === "") return null;
    return new Session(http, s.issuer, s.clientId, this.app.secretStorage, secretId(s.server, s.vault), () => Date.now());
  }

  render(s: Situation): void {
    // Announced once per session: a run held below the horizon (a note
    // left open) resyncs again on every sync until the note closes.
    if (!this.resyncAnnounced && s.last !== null && s.last !== this.latest?.last && s.last.resynced !== null) {
      this.resyncAnnounced = true;
      new Notice(
        s.last.resynced === "history_rewound"
          ? "Kura: the server lost recent history. Nothing was removed here; files it no longer had were pushed again."
          : `Kura: full resync. ${String(s.last.trashed)} files deleted elsewhere were moved to the trash.`,
      );
    }
    this.latest = s;
    const text = label(s);
    this.statusEl?.setText(text);
    this.ribbonEl?.setAttribute("aria-label", text);
    this.tab?.refreshState();
  }

  /** (Re)builds the sync machinery from the settings and starts it. */
  async start(): Promise<void> {
    this.notifier?.stop();
    this.notifier = null;
    if (!this.secretsAvailable()) {
      new Notice("Kura Sync needs Obsidian 1.11.4 or later: update Obsidian.");
      return;
    }
    if (!secureUrl(this.config.server) || !secureUrl(this.config.issuer)) return;
    const session = this.sessionFor();
    this.session = session;
    const s = this.config;
    if (session === null || this.core === null || s.device === "") return;
    const api = new Api({ server: s.server, token: () => session.token(), device: () => s.device, fetch: (u, i) => fetch(u, i), sleep });
    this.api = api;
    const host = new ObsidianHost({
      adapter: this.app.vault.adapter,
      pluginDir: this.manifest.dir ?? `${this.app.vault.configDir}/plugins/weebo-kura`,
      vault: s.vault,
      isOpen: (p) => this.app.workspace.getLeavesOfType("markdown").some((l) => l.view instanceof MarkdownView && l.view.file?.path === p),
      now: () => Date.now(),
      random: random16,
    });
    try {
      this.vaults = await api.vaults();
    } catch {
      // Offline: the controller reports it.
    }
    let vault = this.vaults.find((v) => v.id === s.vault) ?? null;
    const core = this.core;
    const controller = new Controller({
      run: async (state, dryRun, allowDeletions) => {
        // Never sync with guessed chunking: identical content would get
        // another address (review 4 L4). Fetched again until known.
        if (vault === null) {
          this.vaults = await api.vaults();
          vault = this.vaults.find((v) => v.id === s.vault) ?? null;
        }
        if (vault === null) throw new Error("this vault is not visible to you");
        const c = vault.chunking;
        const options = JSON.stringify({ device: s.device, vault: s.vault, ...c, maxFileSize: s.maxFileMiB * 1024 * 1024, dryRun, allowDeletions });
        const notes = await core.sync(host, api.remote(s.vault), state, options);
        if (!dryRun && vault.configSync.length > 0) await this.syncConfig(core, api, vault.configSync, options);
        return notes;
      },
      loadState: () => host.loadState(),
      confirmDeletions: (deletions, files) => new ConfirmModal(this.app, deletions, files).ask(),
      timers,
      onChange: (st) => { this.render(st); },
      signedIn: () => session.signedIn(),
      role: () => vault?.role ?? null,
    });
    this.controller = controller;
    this.notifier = new Notifier({
      open: async () => api.eventsUrl(s.vault, await api.ticket(s.vault)),
      socket: (url) => {
        const ws = new WebSocket(url);
        const sock: SocketLike = { onmessage: null, onclose: null, onopen: null, close: () => { ws.close(); } };
        ws.onmessage = (e: MessageEvent<unknown>) => sock.onmessage?.({ data: e.data });
        ws.onclose = () => sock.onclose?.();
        ws.onopen = () => sock.onopen?.();
        return sock;
      },
      onNotice: () => void controller.sync(),
      onState: (up) => { controller.setOnline(up); },
      timers,
      random: () => Math.random(),
    });
    await controller.sync();
    this.notifier.start();
  }

  /** `.obsidian` for this device's profile (§6.8): its own state, its
   * synced categories only; received plugins wait for agreement. */
  private async syncConfig(core: Core, api: Api, categories: string[], options: string): Promise<void> {
    const plugins = registry(this.app);
    if (plugins === null) return;
    const s = this.config;
    const guard = new PluginGuard(plugins, s.consent, () => this.saveSettings(), (id) => new PluginModal(this.app, id).ask());
    // Never this plugin's own folder, whatever it is named (phase 8 review M4).
    const own = (this.manifest.dir ?? "").slice(this.app.vault.configDir.length + 1).toLowerCase();
    const keep = (p: string): boolean =>
      core.configSynced(p, categories) && (own === "" || !p.toLowerCase().startsWith(`${own}/`));
    const host = new ObsidianHost({
      adapter: this.app.vault.adapter,
      pluginDir: this.manifest.dir ?? `${this.app.vault.configDir}/plugins/weebo-kura`,
      vault: s.vault,
      isOpen: () => false,
      now: () => Date.now(),
      random: random16,
      root: this.app.vault.configDir,
      keep,
      stateName: "config-state",
      beforeWrite: (p) => guard.beforeWrite(p),
    });
    try {
      // A path the categories no longer cover is forgotten here, never
      // deleted on the user's other devices.
      const raw = await host.loadState();
      const state = forget(raw, keep);
      await core.sync(host, api.remote(s.vault, "config"), state, options);
      await guard.review(codeHashes(await host.loadState()));
    } catch (e) {
      new Notice(`Kura: the configuration did not sync: ${String(e)}`);
    }
  }

  async signIn(): Promise<void> {
    const s = this.config;
    if (!this.secretsAvailable()) {
      new Notice("Kura Sync needs Obsidian 1.11.4 or later: update Obsidian.");
      return;
    }
    if (s.server === "" || s.issuer === "") {
      new Notice("Set the server and the identity provider first.");
      return;
    }
    if (!secureUrl(s.server) || !secureUrl(s.issuer)) {
      new Notice("The server and the identity provider must use https://.");
      return;
    }
    try {
      const d = await discover(http, s.issuer);
      const code = await startDevice(http, d, s.clientId);
      const modal = new CodeModal(this.app, code);
      modal.open();
      const tokens = await waitForApproval(http, d, s.clientId, code, sleep, () => Date.now(), () => modal.cancelled);
      modal.close();
      const id = secretId(s.server, s.vault === "" ? "pending" : s.vault);
      const session = new Session(http, s.issuer, s.clientId, this.app.secretStorage, id, () => Date.now());
      session.adopt(tokens);
      const api = new Api({ server: s.server, token: () => session.token(), device: () => null, fetch: (u, i) => fetch(u, i), sleep });
      if (s.device === "") {
        s.deviceName = s.deviceName === "" ? (Platform.isMobile ? "mobile" : "desktop") : s.deviceName;
        s.device = await api.register(s.deviceName, Platform.isMobile ? "mobile" : "desktop");
      }
      this.vaults = await new Api({ server: s.server, token: () => session.token(), device: () => s.device, fetch: (u, i) => fetch(u, i), sleep }).vaults();
      await this.saveSettings();
      // `display` is deprecated from 1.13; the plugin supports 1.11.4 (§6.8).
      // eslint-disable-next-line @typescript-eslint/no-deprecated
      this.tab?.display();
      new Notice("Signed in to Kura. Choose a vault in the settings.");
      await this.start();
    } catch (e) {
      new Notice(e instanceof AuthError ? e.message : `Sign-in failed: ${String(e)}`);
    }
  }

  /** Choosing another vault detaches first: the sync state is dropped, files stay (§4.5). */
  async chooseVault(id: string): Promise<void> {
    const v = this.vaults.find((x) => x.id === id);
    if (v === undefined) return;
    const pending = this.app.secretStorage.getSecret(secretId(this.config.server, "pending"));
    this.config.vault = v.id;
    this.config.vaultName = v.name;
    if (pending !== null && pending !== "") {
      this.app.secretStorage.setSecret(secretId(this.config.server, v.id), pending);
      // Rotation makes a copy stale: never leave one behind (review 4 L4).
      this.app.secretStorage.setSecret(secretId(this.config.server, "pending"), "");
    }
    await this.saveSettings();
    await this.start();
  }

  signOut(): void {
    this.session?.signOut();
    this.app.secretStorage.setSecret(secretId(this.config.server, "pending"), "");
    this.notifier?.stop();
    // eslint-disable-next-line @typescript-eslint/no-deprecated -- supports 1.11.4
    this.tab?.display();
  }
}
