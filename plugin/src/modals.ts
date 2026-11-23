// The plugin's dialogs, drawn the way Obsidian draws its own: text first, then
// one button row, the way out on the left and the action on the right.
import { Modal, setIcon } from "obsidian";
import type { App } from "obsidian";
import type { DeviceCode } from "./auth";
import { t } from "./i18n";
import type { Setup } from "./setup";
import type { Report } from "./types";
import type { Candidate } from "./update";

interface Button {
  text: string;
  cls?: string;
  /** Resolves the question with yes; any other button only closes. */
  yes?: boolean;
  run?: () => void;
}

/** A question with a yes: anything but its button, Escape included, is no. */
abstract class Ask extends Modal {
  private answer: (yes: boolean) => void = () => undefined;

  ask(): Promise<boolean> {
    return new Promise((resolve) => {
      this.answer = resolve;
      this.open();
    });
  }

  protected buttons(...list: Button[]): void {
    const row = this.contentEl.createDiv({ cls: "modal-button-container" });
    for (const b of list) {
      const el = row.createEl("button", { text: b.text, ...(b.cls === undefined ? {} : { cls: b.cls }) });
      el.onclick = () => {
        b.run?.();
        if (b.yes === true) {
          this.answer(true);
          this.answer = () => undefined;
        }
        this.close();
      };
    }
  }

  override onClose(): void {
    this.answer(false);
    this.contentEl.empty();
  }
}

/** The device-code sign-in (§6.8): the page to open, the code, the time left. */
export class CodeModal extends Modal {
  cancelled = false;
  private timer: number | null = null;

  constructor(app: App, private readonly code: DeviceCode, private readonly now: () => number) {
    super(app);
  }

  override onOpen(): void {
    this.setTitle(t("code.title"));
    const { contentEl } = this;
    const complete = this.code.verificationUriComplete;
    contentEl.createEl("p", { text: t("code.step1") });
    contentEl.createEl("p", { text: complete === null ? t("code.step2") : t("code.prefilled") });
    const codeRow = contentEl.createDiv({ cls: "kura-code-row" });
    codeRow.createEl("code", { text: this.code.userCode, cls: "kura-code" });
    const copy = codeRow.createEl("button", { cls: "clickable-icon", attr: { "aria-label": t("code.copy") } });
    setIcon(copy, "copy");
    copy.onclick = () => {
      void navigator.clipboard.writeText(this.code.userCode).then(() => {
        copy.setAttribute("aria-label", t("code.copied"));
        setIcon(copy, "check");
        window.setTimeout(() => {
          copy.setAttribute("aria-label", t("code.copy"));
          setIcon(copy, "copy");
        }, 2000);
      });
    };
    const wait = contentEl.createEl("p", { cls: "kura-wait", attr: { role: "status" } });
    const until = this.now() + this.code.expiresIn * 1000;
    const tick = (): void => {
      const left = Math.max(0, Math.round((until - this.now()) / 1000));
      wait.setText(left === 0 ? t("code.expired") : t("code.waiting", { time: `${String(Math.floor(left / 60))}:${String(left % 60).padStart(2, "0")}` }));
      if (left === 0 && this.timer !== null) window.clearInterval(this.timer);
    };
    tick();
    this.timer = window.setInterval(tick, 1000);
    const row = contentEl.createDiv({ cls: "modal-button-container" });
    row.createEl("button", { text: t("code.cancel") }).onclick = () => {
      this.close();
    };
    const open = row.createEl("button", { text: t("code.open"), cls: "mod-cta" });
    open.onclick = () => window.open(complete ?? this.code.verificationUri);
    open.focus();
  }

  override onClose(): void {
    this.cancelled = true;
    if (this.timer !== null) window.clearInterval(this.timer);
    this.contentEl.empty();
  }
}

/** The §4.5 mock: a synced plugin, or a new version of one, runs only on [Enable]. */
export class PluginModal extends Ask {
  constructor(app: App, private readonly name: string, private readonly version: string) {
    super(app);
  }
  override onOpen(): void {
    this.setTitle(t("plugin.title"));
    this.contentEl.createEl("p", { text: t("plugin.p1", { name: this.name, version: this.version }) });
    this.contentEl.createEl("p", { text: t("plugin.p2") });
    this.buttons({ text: t("plugin.keep") }, { text: t("plugin.enable"), cls: "mod-cta", yes: true });
  }
}

/** A question with more than a yes: each button resolves its own answer;
 * Escape gives the first one, the way out, which is also focused. */
abstract class Choose<T> extends Modal {
  private answer: ((v: T) => void) | null = null;
  protected abstract readonly fallback: T;

  choose(): Promise<T> {
    return new Promise((resolve) => {
      this.answer = resolve;
      this.open();
    });
  }

  protected options(list: { text: string; value: T; cls?: string }[]): void {
    const row = this.contentEl.createDiv({ cls: "modal-button-container" });
    list.forEach((b, i) => {
      const el = row.createEl("button", { text: b.text, ...(b.cls === undefined ? {} : { cls: b.cls }) });
      el.onclick = () => {
        this.answer?.(b.value);
        this.answer = null;
        this.close();
      };
      if (i === 0) window.setTimeout(() => { el.focus(); }, 0);
    });
  }

  override onClose(): void {
    this.answer?.(this.fallback);
    this.answer = null;
    this.contentEl.empty();
  }
}

/** RFC 0010 §4.5: a plugin the vault's managers installed, named with its source. */
export class VaultPluginModal extends Choose<boolean | "never"> {
  protected readonly fallback = false;
  constructor(app: App, private readonly name: string, private readonly version: string, private readonly vault: string, private readonly code: string) {
    super(app);
  }
  override onOpen(): void {
    this.setTitle(t("vplugin.title"));
    this.contentEl.createEl("p", { text: t("vplugin.p1", { name: this.name, version: this.version === "" ? "" : ` ${this.version}`, vault: this.vault }) });
    this.contentEl.createEl("p", { text: t("vplugin.p2") });
    if (this.code !== "") this.contentEl.createEl("p", { text: this.code, cls: "kura-why" });
    this.options([
      { text: t("plugin.keep"), value: false },
      { text: t("vplugin.never"), value: "never" },
      { text: t("plugin.enable"), value: true, cls: "mod-cta" },
    ]);
  }
}

/** RFC 0010 decision 10: the agreed version keeps running until the member agrees. */
export class VaultUpdateModal extends Choose<"use" | "later" | "never"> {
  protected readonly fallback = "later";
  constructor(app: App, private readonly name: string, private readonly from: string, private readonly to: string, private readonly code: string) {
    super(app);
  }
  override onOpen(): void {
    this.setTitle(t("vupdate.title"));
    this.contentEl.createEl("p", { text: t("vupdate.p1", { name: this.name, from: this.from, to: this.to }) });
    this.contentEl.createEl("p", { text: t("vupdate.p2", { from: this.from }) });
    if (this.code !== "") this.contentEl.createEl("p", { text: this.code, cls: "kura-why" });
    this.options([
      { text: t("vupdate.later"), value: "later" },
      { text: t("vplugin.never"), value: "never" },
      { text: t("vupdate.use", { to: this.to }), value: "use", cls: "mod-cta" },
    ]);
  }
}

/** RFC 0010 §4.5: version, source, sizes, hashes and provenance before anything is written. */
export class UpdateModal extends Choose<boolean> {
  protected readonly fallback = false;
  constructor(app: App, private readonly from: string, private readonly c: Candidate) {
    super(app);
  }
  override onOpen(): void {
    this.setTitle(t("upd.title"));
    const c = this.c;
    this.contentEl.createEl("p", { text: t("upd.p1", { from: this.from, to: c.version, source: c.source }) });
    const ul = this.contentEl.createEl("ul", { cls: "kura-list" });
    for (const f of c.files) ul.createEl("li", { text: `${f.name}  ${(f.data.byteLength / 1024).toFixed(1)} KB  sha256 ${f.sha256.slice(0, 4)}…${f.sha256.slice(-4)}` });
    // Decision 13: in words and colour; still allowed when not verified.
    this.contentEl.createEl("p", { text: c.verified ? t("upd.verified") : t("upd.unverified"), cls: c.verified ? "kura-verified" : "mod-warning" });
    this.contentEl.createEl("p", { text: t("upd.kept") });
    this.options([
      { text: t("upd.cancel"), value: false },
      { text: t("upd.go"), value: true, cls: "mod-cta" },
    ]);
  }
}

/** What a setup link would change, shown before anything is applied. */
export class SetupModal extends Ask {
  constructor(app: App, private readonly setup: Setup, private readonly currentServer: string) {
    super(app);
  }
  override onOpen(): void {
    this.setTitle(t("setup.title"));
    this.contentEl.createEl("p", { text: t("setup.p1") });
    const list = this.contentEl.createEl("ul", { cls: "kura-list" });
    list.createEl("li", { text: t("setup.server", { v: this.setup.server }) });
    list.createEl("li", { text: t("setup.issuer", { v: this.setup.issuer }) });
    list.createEl("li", { text: t("setup.vault", { v: this.setup.vault === "" ? t("setup.vaultLater") : this.setup.vaultName }) });
    if (this.currentServer !== "" && this.currentServer !== this.setup.server) {
      this.contentEl.createEl("p", { text: t("setup.replaces", { v: this.currentServer }), cls: "mod-warning" });
    }
    this.buttons({ text: t("setup.cancel") }, { text: t("setup.connect"), cls: "mod-cta", yes: true });
  }
}

/** A full resync would trash many files: the user decides (§6.3). */
export class ConfirmModal extends Ask {
  constructor(app: App, private readonly deletions: number, private readonly files: number) {
    super(app);
  }
  override onOpen(): void {
    this.setTitle(t("trash.title"));
    const share = this.files === 0 ? 100 : Math.round((this.deletions * 100) / this.files);
    this.contentEl.createEl("p", { text: t("trash.p1", { n: this.deletions, share }) });
    this.buttons({ text: t("trash.notNow") }, { text: t("trash.move"), cls: "mod-warning", yes: true });
  }
}

/** Another vault replaces this one's sync state: said before it happens (§4.5). */
export class SwitchModal extends Ask {
  constructor(app: App, private readonly from: string, private readonly to: string) {
    super(app);
  }
  override onOpen(): void {
    this.setTitle(t("switch.title"));
    this.contentEl.createEl("p", { text: t("switch.p1", { from: this.from, to: this.to }) });
    this.buttons({ text: t("switch.cancel") }, { text: t("switch.go"), cls: "mod-cta", yes: true });
  }
}

/** Once, when nothing is set up: the two ways in. */
export class WelcomeModal extends Ask {
  override onOpen(): void {
    this.setTitle(t("welcome.title"));
    this.contentEl.createEl("p", { text: t("welcome.p1") });
    this.contentEl.createEl("p", { text: t("welcome.p2") });
    this.contentEl.createEl("p", { text: t("welcome.p3") });
    this.buttons({ text: t("welcome.later") }, { text: t("welcome.manual"), cls: "mod-cta", yes: true });
  }
}

/** Conflicts and unsyncable files of the last run, each one a click from its file. */
export class ProblemsModal extends Modal {
  constructor(app: App, private readonly report: Report | null, private readonly openFile: (path: string) => void) {
    super(app);
  }
  override onOpen(): void {
    this.setTitle(t("problems.title"));
    renderProblems(this.contentEl, this.report, (p) => {
      this.close();
      this.openFile(p);
    });
  }
  override onClose(): void {
    this.contentEl.empty();
  }
}

/** The two lists, shared by the dialog and the settings tab; nothing when there is nothing. */
export function renderProblems(el: HTMLElement, report: Report | null, openFile: (path: string) => void): boolean {
  const conflicts = report?.conflicts ?? [];
  const unsyncable = [...(report?.unsyncable ?? []), ...(report?.failed ?? [])];
  if (conflicts.length === 0 && unsyncable.length === 0) {
    el.createEl("p", { text: t("problems.none"), cls: "setting-item-description" });
    return false;
  }
  if (conflicts.length > 0) {
    el.createEl("h4", { text: `${t("problems.conflicts")} (${String(conflicts.length)})` });
    el.createEl("p", { text: t("problems.conflictsHelp"), cls: "setting-item-description" });
    const ul = el.createEl("ul", { cls: "kura-list" });
    for (const path of conflicts) {
      const a = ul.createEl("li").createEl("a", { text: path, href: "#" });
      a.onclick = (e) => {
        e.preventDefault();
        openFile(path);
      };
    }
  }
  if (unsyncable.length > 0) {
    el.createEl("h4", { text: `${t("problems.unsyncable")} (${String(unsyncable.length)})` });
    el.createEl("p", { text: t("problems.unsyncableHelp"), cls: "setting-item-description" });
    const ul = el.createEl("ul", { cls: "kura-list" });
    for (const [path, why] of unsyncable) {
      const li = ul.createEl("li");
      li.createSpan({ text: path });
      li.createSpan({ text: ` · ${why}`, cls: "kura-why" });
    }
  }
  return true;
}
