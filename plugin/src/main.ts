// Kura Sync (RFC 0001 §6.8): Obsidian glue only. Sign-in, sync timing,
// notifications, the vault host and the status words live in tested modules.
import { MarkdownView, Menu, Notice, Platform, Plugin, TFile, getLanguage, requestUrl, setIcon, setTooltip } from "obsidian";
import type { App } from "obsidian";
import wasmBytes from "./wasm/kura_bg.wasm";
import { isRecord } from "./types";
import { AuthError, Session, discover, secretId, secureUrl, startDevice, waitForApproval } from "./auth";
import { parseSetup } from "./setup";
import type { Http } from "./auth";
import { Controller } from "./controller";
import { loadCore } from "./core";
import type { Core } from "./core";
import { ObsidianHost } from "./host";
import { Notifier } from "./notifier";
import type { SocketLike } from "./notifier";
import { Api, withDevice } from "./remote";
import type { VaultView } from "./remote";
import { KuraSettingTab } from "./settings";
import type { Situation } from "./status";
import { icon, label, newConflicts, tone, word } from "./status";
import { ago, setLanguage, t } from "./i18n";
import { consoleUrl, entryOf } from "./links";
import { CodeModal, ConfirmModal, PluginModal, ProblemsModal, SetupModal, SwitchModal, WelcomeModal } from "./modals";
import type { Report } from "./types";
import { PluginGuard, codeHashes, emptyConsent, forget, obsidianRegistry, readConsent } from "./plugins";
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
  /** The first-run welcome was shown once. */
  welcomed: boolean;
}

const DEFAULTS: Settings = { server: "", issuer: "", clientId: "kura-plugin", vault: "", vaultName: "", device: "", deviceName: "", maxFileMiB: 200, consent: emptyConsent(), welcomed: false };

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
    welcomed: d.welcomed === true,
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

/** Obsidian's plugin switchboard, resolved folder by folder (plugins.ts). */
function registry(app: App): PluginRegistry | null {
  return obsidianRegistry(Reflect.get(app, "plugins"), app.vault.adapter, app.vault.configDir);
}

const CONFIG_EVERY_MS = 5 * 60 * 1000 - 15_000;

/** Why something failed, in a sentence: the message, not the object. */
function because(e: unknown): string {
  return e instanceof AuthError || e instanceof Error ? e.message : String(e);
}

/** A notice with one button: the thing to do about it, at hand. */
function notify(text: string, action: { label: string; run: () => void }): void {
  const body = createFragment((f) => {
    f.createSpan({ text });
    const b = f.createEl("button", { text: action.label, cls: "kura-notice-action" });
    b.onclick = action.run;
  });
  new Notice(body, 10_000);
}

export default class KuraPlugin extends Plugin {
  config: Settings = { ...DEFAULTS };
  latest: Situation | null = null;
  private resyncAnnounced = false;
  /** When `.obsidian` was last scanned: at most every 5 minutes, and on resume (§6.8). */
  private configSyncedAt = 0;
  vaults: VaultView[] = [];
  private core: Core | null = null;
  private session: Session | null = null;
  private api: Api | null = null;
  private controller: Controller | null = null;
  private notifier: Notifier | null = null;
  private statusEl: HTMLElement | null = null;
  private statusIcon: HTMLElement | null = null;
  private statusText: HTMLElement | null = null;
  private ribbonEl: HTMLElement | null = null;
  private tab: KuraSettingTab | null = null;
  /** The note host of the running sync: its state maps paths to entries. */
  private host: ObsidianHost | null = null;
  /** When the last run finished, on this device, in this session. */
  lastSyncAt: number | null = null;
  private readOnlyAnnounced = false;

  override async onload(): Promise<void> {
    setLanguage(getLanguage());
    this.config = readSettings(await this.loadData());
    this.core = loadCore(wasmBytes);
    if (!Platform.isMobile) {
      // The state, as an icon and its word; a click opens what can be done about it.
      const el = this.addStatusBarItem();
      el.addClass("kura-status", "mod-clickable");
      this.statusIcon = el.createSpan({ cls: "kura-status-icon" });
      this.statusText = el.createSpan();
      el.onClickEvent((e) => { this.menu(e); });
      this.statusEl = el;
    }
    // Mobile has no plugin status bar: the ribbon's icon and label carry the state (§4.5).
    this.ribbonEl = this.addRibbonIcon("cloud", "Kura", (e) => { this.menu(e); });
    this.tab = new KuraSettingTab(this.app, this);
    this.addSettingTab(this.tab);
    this.commands();
    // The console's "Open in Obsidian": obsidian://kura-setup?server=…&issuer=…&kuraVault=…
    this.registerObsidianProtocolHandler("kura-setup", (params) => void this.setupFrom(params));
    this.registerEvent(
      this.app.workspace.on("file-menu", (menu, file) => {
        if (!(file instanceof TFile) || !this.connected()) return;
        menu.addItem((i) => i.setSection("kura").setTitle(t("menu.openNote")).setIcon("globe").onClick(() => void this.openInConsole(file.path, false)));
        menu.addItem((i) => i.setSection("kura").setTitle(t("menu.history")).setIcon("history").onClick(() => void this.openInConsole(file.path, true)));
        menu.addItem((i) => i.setSection("kura").setTitle(t("menu.copyLink")).setIcon("link").onClick(() => void this.copyLink(file.path)));
      }),
    );
    const touched = (): void => this.controller?.touched();
    this.registerEvent(this.app.vault.on("create", touched));
    this.registerEvent(this.app.vault.on("modify", touched));
    this.registerEvent(this.app.vault.on("delete", touched));
    this.registerEvent(this.app.vault.on("rename", touched));
    this.registerInterval(window.setInterval(() => void this.controller?.sync(), 5 * 60 * 1000));
    // The "2 min ago" of the tooltip and the settings ages with the clock.
    this.registerInterval(window.setInterval(() => { if (this.latest !== null) this.paint(this.latest); }, 60 * 1000));
    // Resume: pull when the app comes back (§6.3), and scan the configuration (§6.8).
    this.registerDomEvent(document, "visibilitychange", () => {
      if (document.visibilityState === "visible") {
        this.configSyncedAt = 0;
        void this.controller?.sync();
      }
    });
    this.app.workspace.onLayoutReady(() => {
      void this.start();
      void this.welcome();
    });
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

  /** Signed in, with a vault: the console links mean something. */
  connected(): boolean {
    return this.config.device !== "" && this.config.vault !== "" && this.config.server !== "";
  }

  private pluginDir(): string {
    return this.manifest.dir ?? `${this.app.vault.configDir}/plugins/weebo-kura`;
  }

  private sessionFor(): Session | null {
    const s = this.config;
    if (!this.secretsAvailable() || s.server === "" || s.issuer === "" || s.vault === "") return null;
    return new Session(http, s.issuer, s.clientId, this.app.secretStorage, secretId(s.server, s.vault), () => Date.now());
  }

  private commands(): void {
    this.addCommand({ id: "sync-now", name: t("cmd.sync"), callback: () => void this.controller?.sync(true) });
    this.addCommand({ id: "sign-in", name: t("cmd.signIn"), callback: () => void this.signIn() });
    this.addCommand({ id: "sign-out", name: t("cmd.signOut"), checkCallback: (check) => {
      if (this.config.device === "") return false;
      if (!check) this.signOut();
      return true;
    } });
    this.addCommand({ id: "status", name: t("cmd.status"), callback: () => {
      new Notice(t("n.status", { word: this.latest === null ? t("word.notConfigured") : word(this.latest), last: this.lastLine() }));
    } });
    this.addCommand({ id: "problems", name: t("cmd.problems"), callback: () => { this.showProblems(); } });
    this.addCommand({ id: "settings", name: t("cmd.settings"), callback: () => { this.openSettings(); } });
    const onNote = (run: (path: string) => void) => (check: boolean): boolean => {
      const file = this.app.workspace.getActiveFile();
      if (file === null || !this.connected()) return false;
      if (!check) run(file.path);
      return true;
    };
    this.addCommand({ id: "open-note", name: t("cmd.openNote"), checkCallback: onNote((p) => void this.openInConsole(p, false)) });
    this.addCommand({ id: "note-history", name: t("cmd.history"), checkCallback: onNote((p) => void this.openInConsole(p, true)) });
    this.addCommand({ id: "copy-link", name: t("cmd.copyLink"), checkCallback: onNote((p) => void this.copyLink(p)) });
  }

  /** What the status bar and the ribbon offer: the next useful things to do. */
  private menu(e: MouseEvent): void {
    const m = new Menu();
    if (this.latest?.signedIn !== true) {
      m.addItem((i) => i.setTitle(t("menu.signIn")).setIcon("log-in").onClick(() => void this.signIn()));
    } else {
      m.addItem((i) => i.setTitle(t("menu.sync")).setIcon("refresh-cw").onClick(() => void this.controller?.sync(true)));
    }
    const last = this.latest?.last ?? null;
    if ((last?.conflicts.length ?? 0) + (last?.unsyncable.length ?? 0) + (last?.failed.length ?? 0) > 0) {
      m.addItem((i) => i.setTitle(t("menu.problems")).setIcon("alert-triangle").onClick(() => { this.showProblems(); }));
    }
    if (this.connected()) {
      m.addSeparator();
      const file = this.app.workspace.getActiveFile();
      if (file !== null) m.addItem((i) => i.setTitle(t("menu.openNote")).setIcon("globe").onClick(() => void this.openInConsole(file.path, false)));
      m.addItem((i) => i.setTitle(t("menu.openVault")).setIcon("external-link").onClick(() => window.open(consoleUrl(this.config.server, this.config.vault))));
    }
    m.addSeparator();
    m.addItem((i) => i.setTitle(t("menu.settings")).setIcon("settings").onClick(() => { this.openSettings(); }));
    m.showAtMouseEvent(e);
  }

  /** "Last sync 2 min ago", or that it has not happened yet. */
  lastLine(): string {
    return this.lastSyncAt === null ? t("tip.never") : t("tip.last", { when: ago(this.lastSyncAt, Date.now()) });
  }

  async syncNow(): Promise<void> {
    await this.controller?.sync(true);
  }

  showProblems(): void {
    new ProblemsModal(this.app, this.latest?.last ?? null, (p) => { this.openFile(p); }).open();
  }

  openFile(path: string): void {
    void this.app.workspace.openLinkText(path, "", false);
  }

  /** Obsidian's settings, on this plugin's tab (not public API: read defensively). */
  openSettings(): void {
    const setting: unknown = Reflect.get(this.app, "setting");
    if (typeof setting !== "object" || setting === null) return;
    const open: unknown = Reflect.get(setting, "open");
    const tab: unknown = Reflect.get(setting, "openTabById");
    if (typeof open === "function") Reflect.apply(open, setting, []);
    if (typeof tab === "function") Reflect.apply(tab, setting, [this.manifest.id]);
  }

  private async entry(path: string): Promise<string | null> {
    const host = this.host;
    return host === null ? null : entryOf(await host.loadState(), path);
  }

  private async openInConsole(path: string, history: boolean): Promise<void> {
    const entry = await this.entry(path);
    if (entry === null) {
      new Notice(t("n.notSynced"));
      return;
    }
    window.open(consoleUrl(this.config.server, this.config.vault, entry, history));
  }

  private async copyLink(path: string): Promise<void> {
    const entry = await this.entry(path);
    if (entry === null) {
      new Notice(t("n.notSynced"));
      return;
    }
    await navigator.clipboard.writeText(consoleUrl(this.config.server, this.config.vault, entry));
    new Notice(t("n.copied"));
  }

  /** Once, on a vault with nothing set up: how to connect it. */
  private async welcome(): Promise<void> {
    if (this.config.welcomed || this.config.server !== "") return;
    this.config.welcomed = true;
    await this.saveSettings();
    if (await new WelcomeModal(this.app).ask()) this.openSettings();
  }

  render(s: Situation): void {
    const before = this.latest?.last ?? null;
    if (s.last !== null && s.last !== before) {
      this.lastSyncAt = Date.now();
      // Announced once per session: a run held below the horizon (a note
      // left open) resyncs again on every sync until the note closes.
      if (!this.resyncAnnounced && s.last.resynced !== null) {
        this.resyncAnnounced = true;
        new Notice(s.last.resynced === "history_rewound" ? t("n.rewound") : t("n.resync", { n: s.last.trashed }));
      }
      this.announceConflicts(before, s.last);
    }
    if (!this.readOnlyAnnounced && s.role === "viewer" && s.pending > 0) {
      this.readOnlyAnnounced = true;
      new Notice(t("n.readOnly"), 10_000);
    }
    this.latest = s;
    this.paint(s);
  }

  private announceConflicts(before: Report | null, now: Report): void {
    const fresh = newConflicts(before, now);
    const first = fresh[0];
    if (first === undefined) return;
    const text = fresh.length === 1 ? t("n.conflict", { name: first }) : t("n.conflicts", { n: fresh.length });
    notify(text, fresh.length === 1 ? { label: t("a.open"), run: () => { this.openFile(first); } } : { label: t("a.show"), run: () => { this.showProblems(); } });
  }

  /** The status bar, the ribbon and the settings' first line, from one state. */
  private paint(s: Situation): void {
    const text = label(s);
    const glyph = icon(tone(s));
    const tip = s.pending > 0 ? `${this.lastLine()} · ${t("tip.pending", { n: s.pending })}` : this.lastLine();
    if (this.statusEl !== null && this.statusIcon !== null) {
      setIcon(this.statusIcon, glyph);
      this.statusText?.setText(text);
      this.statusEl.dataset.tone = tone(s);
      setTooltip(this.statusEl, tip, { placement: "top" });
    }
    if (this.ribbonEl !== null) {
      setIcon(this.ribbonEl, glyph);
      this.ribbonEl.dataset.tone = tone(s);
      this.ribbonEl.setAttribute("aria-label", `${text} · ${tip}`);
    }
    this.tab?.refreshState();
  }

  /** (Re)builds the sync machinery from the settings and starts it. */
  async start(): Promise<void> {
    this.notifier?.stop();
    this.notifier = null;
    if (!this.secretsAvailable()) {
      new Notice(t("n.needsObsidian"));
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
      pluginDir: this.pluginDir(),
      vault: s.vault,
      isOpen: (p) => this.app.workspace.getLeavesOfType("markdown").some((l) => l.view instanceof MarkdownView && l.view.file?.path === p),
      now: () => Date.now(),
      random: random16,
    });
    try {
      // A device this server forgot is registered again before any sync.
      this.vaults = await withDevice(() => api.vaults(), this.registrar(session));
    } catch {
      // Offline: the controller reports it.
    }
    this.host = host;
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
        // The notes sync their folders too (RFC 0007); syncConfig builds its own options.
        const options = JSON.stringify({ device: s.device, vault: s.vault, ...c, maxFileSize: s.maxFileMiB * 1024 * 1024, dryRun, allowDeletions, folders: true });
        const notes = await core.sync(host, api.remote(s.vault), state, options);
        // RFC 0006 phase 3: the console shows what this run left undone; a
        // failed report never fails the sync.
        const counts = isRecord(notes) && isRecord(notes.report) && isRecord(notes.report.counts) ? notes.report.counts : null;
        if (!dryRun && counts !== null) void api.report(s.vault, counts).catch(() => undefined);
        // Not on every note sync (review 2026-11-15 #20); the 5-minute timer
        // finds it due, a few seconds early at most.
        if (!dryRun && vault.configSync.length > 0 && Date.now() - this.configSyncedAt >= CONFIG_EVERY_MS) {
          this.configSyncedAt = Date.now();
          await this.syncConfig(core, api, vault.configSync, options);
        }
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
    const guard = new PluginGuard(plugins, s.consent, () => this.saveSettings(), (id) => {
      const manifests: unknown = Reflect.get(Reflect.get(this.app, "plugins") ?? {}, "manifests");
      const m: unknown = isRecord(manifests) ? manifests[id] : null;
      const name = isRecord(m) && typeof m.name === "string" ? m.name : id;
      const version = isRecord(m) && typeof m.version === "string" ? ` ${m.version}` : "";
      return new PluginModal(this.app, name, version).ask();
    });
    // Never this plugin's own folder, whatever it is named (phase 8 review M4).
    const own = (this.manifest.dir ?? "").slice(this.app.vault.configDir.length + 1).toLowerCase();
    const keep = (p: string): boolean =>
      core.configSynced(p, categories) && (own === "" || !p.toLowerCase().startsWith(`${own}/`));
    const host = new ObsidianHost({
      adapter: this.app.vault.adapter,
      pluginDir: this.pluginDir(),
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
      // The configuration space syncs files only (RFC 0007 §4.3).
      await core.sync(host, api.remote(s.vault, "config"), state, options.replace('"folders":true', '"folders":false'));
      await guard.review(codeHashes(await host.loadState()));
    } catch (e) {
      new Notice(t("n.configFailed", { cause: because(e) }));
    }
  }

  /** A session is held for the chosen vault (the settings offer sign-in otherwise). */
  signedIn(): boolean {
    return this.session?.signedIn() ?? false;
  }

  async signIn(): Promise<void> {
    const s = this.config;
    if (!this.secretsAvailable()) {
      new Notice(t("n.needsObsidian"));
      return;
    }
    if (s.server === "" || s.issuer === "") {
      new Notice(t("n.setFirst"));
      return;
    }
    if (!secureUrl(s.server) || !secureUrl(s.issuer)) {
      new Notice(t("n.https"));
      return;
    }
    try {
      const d = await discover(http, s.issuer);
      const code = await startDevice(http, d, s.clientId);
      const modal = new CodeModal(this.app, code, () => Date.now());
      modal.open();
      const tokens = await waitForApproval(http, d, s.clientId, code, sleep, () => Date.now(), () => modal.cancelled);
      modal.close();
      const id = secretId(s.server, s.vault === "" ? "pending" : s.vault);
      const session = new Session(http, s.issuer, s.clientId, this.app.secretStorage, id, () => Date.now());
      session.adopt(tokens);
      const register = this.registrar(session);
      if (s.device === "") await register();
      // A device another server registered (a stale data.json) is replaced.
      const api = new Api({ server: s.server, token: () => session.token(), device: () => s.device, fetch: (u, i) => fetch(u, i), sleep });
      this.vaults = await withDevice(() => api.vaults(), register);
      await this.saveSettings();
      // `display` is deprecated from 1.13; the plugin supports 1.11.4 (§6.8).
      // eslint-disable-next-line @typescript-eslint/no-deprecated
      this.tab?.display();
      new Notice(this.vaults.some((v) => v.id === s.vault) ? t("n.signedInSyncing", { vault: s.vaultName }) : t("n.signedInChoose"));
      await this.start();
    } catch (e) {
      new Notice(t("n.signInFailed", { cause: because(e) }));
    }
  }

  /** A setup link: checked, shown, and applied only when accepted; then the sign-in. */
  async setupFrom(params: Record<string, string | undefined>): Promise<void> {
    const setup = parseSetup(params);
    if (typeof setup === "string") {
      new Notice(t("n.linkRefused", { why: setup }));
      return;
    }
    if (!(await new SetupModal(this.app, setup, this.config.server).ask())) return;
    const s = this.config;
    if (s.server !== setup.server) {
      // A device belongs to one server: a new server registers a new one.
      this.signOut();
      s.device = "";
    }
    s.server = setup.server;
    s.issuer = setup.issuer;
    s.clientId = setup.clientId;
    if (setup.vault !== "") {
      s.vault = setup.vault;
      s.vaultName = setup.vaultName;
    }
    await this.saveSettings();
    await this.signIn();
  }

  /** Choosing another vault detaches first: the sync state is dropped, files stay (§4.5). False when the user kept the vault they had. */
  async chooseVault(id: string): Promise<boolean> {
    const v = this.vaults.find((x) => x.id === id);
    if (v === undefined) return false;
    if (this.config.vault !== "" && this.config.vault !== id && !(await new SwitchModal(this.app, this.config.vaultName, v.name).ask())) return false;
    // The session goes with the device, not the vault: carried from where it
    // is stored now (the vault left, or "pending" before the first choice).
    const from = secretId(this.config.server, this.config.vault === "" ? "pending" : this.config.vault);
    const carried = this.app.secretStorage.getSecret(from);
    this.config.vault = v.id;
    this.config.vaultName = v.name;
    const to = secretId(this.config.server, v.id);
    if (carried !== null && carried !== "" && from !== to) {
      this.app.secretStorage.setSecret(to, carried);
      // Rotation makes a copy stale: never leave one behind (review 4 L4).
      this.app.secretStorage.setSecret(from, "");
    }
    await this.saveSettings();
    await this.start();
    return true;
  }

  /** Registers this device on the configured server and keeps its id. */
  private registrar(session: Session): () => Promise<void> {
    return async () => {
      const s = this.config;
      const kind = Platform.isMobile ? "mobile" : "desktop";
      s.deviceName = s.deviceName === "" ? kind : s.deviceName;
      const api = new Api({ server: s.server, token: () => session.token(), device: () => null, fetch: (u, i) => fetch(u, i), sleep });
      s.device = await api.register(s.deviceName, kind);
      await this.saveSettings();
    };
  }

  /** A device belongs to one server: another server forgets it, and stops
   * the sync with the old one, so the next sign-in registers one there. */
  forgetDevice(): void {
    this.notifier?.stop();
    this.notifier = null;
    this.controller = null;
    this.config.device = "";
  }

  signOut(): void {
    this.session?.signOut();
    this.app.secretStorage.setSecret(secretId(this.config.server, "pending"), "");
    this.notifier?.stop();
    // Repaints the state as signed out.
    void this.controller?.sync();
    // eslint-disable-next-line @typescript-eslint/no-deprecated -- supports 1.11.4
    this.tab?.display();
  }
}
