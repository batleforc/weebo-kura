// The settings panel of §4.5, read top to bottom as a state: where this vault
// syncs and as whom, then the sync itself, its problems, and what few knobs
// there are. The raw connection fields fold away once a link has filled them.
import { Notice, PluginSettingTab, Setting } from "obsidian";
import type { App } from "obsidian";
import { ago, roleName, t } from "./i18n";
import { consoleUrl } from "./links";
import type KuraPlugin from "./main";
import { renderProblems } from "./modals";
import type { Shares } from "./remote";
import { word } from "./status";

export class KuraSettingTab extends PluginSettingTab {
  private stateEl: HTMLElement | null = null;
  private syncEl: HTMLElement | null = null;

  constructor(app: App, private readonly plugin: KuraPlugin) {
    super(app, plugin);
  }

  /** The two live lines, repainted on every state without rebuilding the tab. */
  refreshState(): void {
    const p = this.plugin;
    const s = p.latest;
    this.stateEl?.setText(t("s.state", { word: s === null ? t("word.notConfigured") : word(s) }));
    if (this.syncEl === null) return;
    const last = s?.last ?? null;
    const lines = [
      last === null || p.lastSyncAt === null ? t("s.lastNever") : t("s.last", { when: ago(p.lastSyncAt, Date.now()), pushed: last.pushed, pulled: last.pulled }),
      ...((s?.pending ?? 0) > 0 ? [t("s.pending", { n: s?.pending ?? 0 })] : []),
    ];
    this.syncEl.setText(lines.join(" "));
  }

  override display(): void {
    const { containerEl } = this;
    const p = this.plugin;
    const c = p.config;
    containerEl.empty();
    this.stateEl = containerEl.createEl("p", { cls: "kura-status kura-state", attr: { role: "status" } });
    this.versionLine(containerEl);

    // ── Connection ──
    new Setting(containerEl).setName(t("s.connection")).setHeading();
    const configured = c.server !== "" && c.issuer !== "";
    // Signed in means a session is held, not only a device registered: a
    // device whose session is gone must be offered sign-in again.
    const signedIn = c.device !== "" && p.signedIn();
    const account = new Setting(containerEl).setDesc(
      !configured ? t("s.notConfigured") : !signedIn ? t("s.notSignedIn") : t("s.connectedAs", { server: hostOf(c.server), device: c.deviceName }),
    );
    if (configured && !signedIn) account.addButton((b) => b.setButtonText(t("s.signIn")).setCta().onClick(() => void p.signIn()));
    if (signedIn) account.addButton((b) => b.setButtonText(t("s.signOut")).onClick(() => { p.signOut(); }));

    const manual = containerEl.createEl("details", { cls: "kura-manual" });
    // Open until something is set: then the link's values need no second look.
    manual.open = !configured;
    manual.createEl("summary", { text: t("s.manual") });
    new Setting(manual).setName(t("s.server")).addText((x) =>
      x.setPlaceholder("https://kura.example.org").setValue(c.server).onChange(async (v) => {
        // A device belongs to one server, as with a setup link.
        if (v.trim() !== c.server && c.device !== "") p.forgetDevice();
        c.server = v.trim();
        await p.saveSettings();
      }),
    );
    new Setting(manual).setName(t("s.issuer")).setDesc(t("s.issuerDesc")).addText((x) =>
      x.setPlaceholder("https://auth.example.org/application/o/kura/").setValue(c.issuer).onChange(async (v) => {
        c.issuer = v.trim();
        await p.saveSettings();
      }),
    );
    new Setting(manual).setName(t("s.client")).addText((x) =>
      x.setValue(c.clientId).onChange(async (v) => {
        c.clientId = v.trim() === "" ? "kura-plugin" : v.trim();
        await p.saveSettings();
      }),
    );
    // The server knows the device by the name it registered with: fixed once signed in.
    new Setting(manual).setName(t("s.deviceName")).setDesc(t("s.deviceNameDesc")).addText((x) =>
      x.setPlaceholder("desktop").setValue(c.deviceName).setDisabled(c.device !== "").onChange(async (v) => {
        c.deviceName = v.trim().slice(0, 64);
        await p.saveSettings();
      }),
    );
    if (!configured) manual.createDiv({ cls: "kura-manual-actions" }).createEl("button", { text: t("s.signIn"), cls: "mod-cta" }).onclick = () => void p.signIn();

    // ── Vault ──
    new Setting(containerEl).setName(t("s.vault")).setHeading();
    const vault = new Setting(containerEl).setName(t("s.vault")).setDesc(p.vaults.length === 0 ? t("s.vaultNone") : t("s.vaultDesc"));
    if (p.vaults.length > 0) {
      vault.addDropdown((d) => {
        if (c.vault === "") d.addOption("", "—");
        for (const v of p.vaults) d.addOption(v.id, t("s.role", { name: v.name, role: roleName(v.role) }));
        d.setValue(c.vault).onChange(async (id) => {
          // Kept: the dropdown goes back to the vault that still syncs.
          if (!(await p.chooseVault(id))) d.setValue(c.vault);
        });
      });
    }
    if (p.connected()) {
      vault.addExtraButton((b) => b.setIcon("external-link").setTooltip(t("s.openConsole")).onClick(() => window.open(consoleUrl(c.server, c.vault))));
    }

    // ── Synchronisation ──
    new Setting(containerEl).setName(t("s.sync")).setHeading();
    const sync = new Setting(containerEl);
    this.syncEl = sync.descEl;
    if (p.connected()) sync.addButton((b) => b.setButtonText(t("s.syncNow")).onClick(() => void p.syncNow()));
    this.refreshState();

    // ── Problems: only when there are some ──
    const last = p.latest?.last ?? null;
    if ((last?.conflicts.length ?? 0) + (last?.unsyncable.length ?? 0) + (last?.failed.length ?? 0) > 0) {
      new Setting(containerEl).setName(t("s.problems")).setHeading();
      renderProblems(containerEl.createDiv({ cls: "kura-problems" }), last, (path) => { p.openFile(path); });
    }

    // ── Advanced ──
    new Setting(containerEl).setName(t("s.advanced")).setHeading();
    new Setting(containerEl).setName(t("s.maxSize")).setDesc(t("s.maxSizeDesc")).addText((x) => {
      x.inputEl.type = "number";
      x.inputEl.min = "1";
      x.setValue(String(c.maxFileMiB)).onChange(async (v) => {
        const n = Number(v);
        if (Number.isInteger(n) && n > 0) {
          c.maxFileMiB = n;
          await p.saveSettings();
        }
      });
    });
    const synced = p.vaults.find((v) => v.id === c.vault)?.configSync ?? [];
    new Setting(containerEl).setName(t("s.configSync")).setDesc(t("s.configSyncDesc", { list: synced.length === 0 ? t("s.configSyncNone") : synced.join(", ") }));
    if (p.connected() && synced.length > 0) void this.syncedSettings(containerEl.createDiv(), synced);
  }

  /** RFC 0010 §4.5: this build against the server's, the update at hand, and the way back. */
  private versionLine(el: HTMLElement): void {
    const p = this.plugin;
    const server = p.serverVersion();
    const line = new Setting(el).setName(server === null ? t("s.versionAlone", { client: p.version }) : t("s.version", { client: p.version, server }));
    const d = p.drift();
    if (d === "older" && server !== null) {
      line.setDesc(t("s.updateAvailable"));
      line.addButton((b) => b.setButtonText(t("s.updateTo", { v: server })).setCta().onClick(async () => {
        const release = await p.update();
        // No build on the server (its console is off): its release page instead.
        if (release !== null) line.setDesc(t("s.updateRelease", { url: release }));
      }));
    } else if (d === "newer") {
      line.setDesc(t("s.serverOlder"));
    }
    void p.previous().then((v) => {
      if (v !== null && v !== p.version) line.addButton((b) => b.setButtonText(t("s.goBack", { v })).onClick(() => void p.goBack()));
    });
  }

  /** RFC 0010 §4.5 "Synced settings": the member's toggles, then the vault's plugins. */
  private async syncedSettings(el: HTMLElement, categories: string[]): Promise<void> {
    const p = this.plugin;
    let shares: Shares;
    try {
      shares = await p.shares();
    } catch (e) {
      el.createEl("p", { text: t("s.sharedFailed", { cause: e instanceof Error ? e.message : String(e) }), cls: "setting-item-description" });
      return;
    }
    const again = (): void => {
      el.empty();
      void this.syncedSettings(el, categories);
    };
    const manager = p.isManager();
    const onVault = new Set(shares.vaultPlugins);
    const local = categories.includes("plugins") ? await p.localPlugins() : [];
    const items = [
      ...local.filter((m) => !onVault.has(m.folder.toLowerCase())).map((m) => ({ item: `plugin:${m.folder.toLowerCase()}`, label: `${m.name} ${m.version}`.trim(), folder: m.folder })),
      ...(["snippets", "themes", "hotkeys"] as const).filter((c) => categories.includes(c)).map((c) => ({ item: c, label: t(`item.${c}`), folder: null })),
    ];
    new Setting(el).setDesc(t("s.sharedSet", { list: categories.map((c) => (c === "plugins" || c === "snippets" || c === "themes" || c === "hotkeys" ? t(`item.${c}`) : c)).join(", ") }));
    if (items.length > 0) new Setting(el).setName(t("s.sharedTitle")).setHeading();
    for (const { item, label, folder } of items) {
      const choice = shares.choices[item];
      const byDefault = shares.defaults.includes(item);
      const row = new Setting(el)
        .setName(folder !== null && manager ? `${label} ${t("s.yours")}` : label)
        .setDesc(choice === undefined ? (byDefault ? t("s.sharedDefault") : "") : byDefault && !choice ? t("s.sharedOff") : "");
      row.addToggle((tg) => tg.setValue(shares.shared.includes(item)).onChange(async (v) => {
        try {
          await p.setShare(item, v);
        } catch (e) {
          new Notice(t("s.sharedFailed", { cause: e instanceof Error ? e.message : String(e) }));
        }
        again();
      }));
      if (choice !== undefined) {
        row.addExtraButton((b) => b.setIcon("rotate-ccw").setTooltip(t("s.sharedReset")).onClick(async () => {
          await p.setShare(item, null).catch(() => undefined);
          again();
        }));
      }
      if (folder !== null && manager) row.addButton((b) => b.setButtonText(t("s.install")).onClick(async () => { await p.installOnVault(folder); again(); }));
    }
    // The vault's plugins and the ones declined for good here, with their state on this device.
    const ids = [...new Set([...shares.vaultPlugins, ...p.config.consent.never])];
    if (ids.length === 0) return;
    new Setting(el).setName(t("s.vaultPlugins")).setHeading();
    const words = { enabled: t("s.vpEnabled"), disabled: t("s.vpDisabled"), pending: t("s.vpPending"), never: t("s.vpNever"), missing: t("s.vpMissing") };
    for (const id of ids) {
      const m = await p.localManifest(id);
      const state = await p.vaultPluginState(id);
      const row = new Setting(el).setName(`${m.name} ${m.version}`.trim() + (manager ? ` ${t("s.onVault")}` : "")).setDesc(words[state]);
      if (state === "pending") row.addButton((b) => b.setButtonText(t("s.vpReview")).onClick(() => void p.reviewUpdate(id)));
      if (state === "never") row.addButton((b) => b.setButtonText(t("s.vpAllow")).onClick(async () => { await p.allowAgain(id); again(); }));
      if (manager && onVault.has(id)) {
        if (state !== "missing" && state !== "never") row.addButton((b) => b.setButtonText(t("s.updateVault")).onClick(async () => { await p.installOnVault(id); again(); }));
        row.addButton((b) => b.setButtonText(t("s.remove")).onClick(async () => { await p.removeFromVault(id); again(); }));
      }
    }
  }
}

/** A server's host, for a sentence: the full URL lives in the manual fields. */
function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}
