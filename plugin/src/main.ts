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
}

const DEFAULTS: Settings = { server: "", issuer: "", clientId: "kura-plugin", vault: "", vaultName: "", device: "", deviceName: "", maxFileMiB: 200 };

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

export default class KuraPlugin extends Plugin {
  config: Settings = { ...DEFAULTS };
  latest: Situation | null = null;
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
      void this.controller?.sync();
    });
    this.tab = new KuraSettingTab(this.app, this);
    this.addSettingTab(this.tab);
    this.addCommand({ id: "sync-now", name: "Sync now", callback: () => void this.controller?.sync() });
    this.addCommand({ id: "sign-in", name: "Sign in", callback: () => void this.signIn() });
    const touched = (): void => this.controller?.touched();
    this.registerEvent(this.app.vault.on("create", touched));
    this.registerEvent(this.app.vault.on("modify", touched));
    this.registerEvent(this.app.vault.on("delete", touched));
    this.registerEvent(this.app.vault.on("rename", touched));
    // Resume: pull when the app comes back (§6.3).
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
      run: async (state, dryRun) => {
        // Never sync with guessed chunking: identical content would get
        // another address (review 4 L4). Fetched again until known.
        if (vault === null) {
          this.vaults = await api.vaults();
          vault = this.vaults.find((v) => v.id === s.vault) ?? null;
        }
        if (vault === null) throw new Error("this vault is not visible to you");
        const c = vault.chunking;
        const options = JSON.stringify({ device: s.device, vault: s.vault, ...c, maxFileSize: s.maxFileMiB * 1024 * 1024, dryRun });
        return core.sync(host, api.remote(s.vault), state, options);
      },
      loadState: () => host.loadState(),
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
