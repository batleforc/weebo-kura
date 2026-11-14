// The settings panel of §4.5: the state first, then the connection, the
// vault, the per-device size limit, the conflicts and unsyncable files.
import { PluginSettingTab, Setting } from "obsidian";
import type { App } from "obsidian";
import type KuraPlugin from "./main";
import { word } from "./status";

export class KuraSettingTab extends PluginSettingTab {
  private stateEl: HTMLElement | null = null;

  constructor(app: App, private readonly plugin: KuraPlugin) {
    super(app, plugin);
  }

  refreshState(): void {
    const s = this.plugin.latest;
    this.stateEl?.setText(`State: ${s === null ? "not configured" : word(s)}`);
  }

  override display(): void {
    const { containerEl } = this;
    const p = this.plugin;
    containerEl.empty();
    this.stateEl = containerEl.createEl("p", { cls: "kura-status" });
    this.refreshState();
    new Setting(containerEl).setName("Server").addText((t) =>
      t.setPlaceholder("https://kura.example.org").setValue(p.config.server).onChange(async (v) => {
        p.config.server = v.trim();
        await p.saveSettings();
      }),
    );
    new Setting(containerEl).setName("Identity provider").setDesc("The OIDC issuer URL").addText((t) =>
      t.setPlaceholder("https://auth.example.org/application/o/kura/").setValue(p.config.issuer).onChange(async (v) => {
        p.config.issuer = v.trim();
        await p.saveSettings();
      }),
    );
    new Setting(containerEl)
      .setName("Account")
      .setDesc(p.config.device === "" ? "Not signed in" : `device "${p.config.deviceName}"`)
      .addButton((b) => b.setButtonText("Sign in").onClick(() => void p.signIn()))
      .addButton((b) => b.setButtonText("Sign out").onClick(() => { p.signOut(); }));
    const vault = new Setting(containerEl).setName("Vault");
    vault.addDropdown((d) => {
      d.addOption("", "—");
      for (const v of p.vaults) d.addOption(v.id, `${v.name} (${v.role})`);
      d.setValue(p.config.vault).onChange((id) => void p.chooseVault(id));
    });
    new Setting(containerEl).setName("Skip files larger than (MiB)").setDesc("On this device only").addText((t) =>
      t.setValue(String(p.config.maxFileMiB)).onChange(async (v) => {
        const n = Number(v);
        if (Number.isInteger(n) && n > 0) {
          p.config.maxFileMiB = n;
          await p.saveSettings();
        }
      }),
    );
    const last = p.latest?.last ?? null;
    const conflicts = last?.conflicts ?? [];
    containerEl.createEl("h3", { text: `Conflicts (${String(conflicts.length)})` });
    const cl = containerEl.createEl("ul", { cls: "kura-list" });
    for (const c of conflicts) cl.createEl("li", { text: c });
    const unsyncable = [...(last?.unsyncable ?? []), ...(last?.failed ?? [])];
    containerEl.createEl("h3", { text: `Unsyncable files (${String(unsyncable.length)})` });
    const ul = containerEl.createEl("ul", { cls: "kura-list" });
    for (const [path, why] of unsyncable) ul.createEl("li", { text: `${path} - ${why}` });
  }
}
