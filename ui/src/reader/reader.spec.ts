import { mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";

import type { EntryState } from "@/api/generated/types.gen";
import { theme } from "@/prefs";
import { BRIDGE, bridgeHash } from "./bridge";
import { parseRecords } from "./content";
import NoteFrame from "./NoteFrame.vue";
import { ENTRY_LINK, frameDocument, MAX_IMAGE, renderNote } from "./render";
import { ago, day } from "./time";
import {
  buildTree,
  folderNote,
  foldersOf,
  noteName,
  notesInOrder,
  resolver,
} from "./tree";

const entry = (path: string, id = path, size = 10): EntryState => ({
  entry: id,
  path,
  revision: `r-${id}`,
  seq: 1,
  size,
  deleted: false,
  blob: `b-${id}`,
});
const listed = [
  entry("Sessions/Session 11.md", "s11"),
  entry("Sessions/map.png", "map"),
  entry("Resumes/s12.md", "r12"),
];

describe("tree", () => {
  it("folds entries into sorted folders, skipping deleted ones", () => {
    const tree = buildTree([
      ...listed,
      { ...entry("Old/x.md"), deleted: true },
    ]);
    expect(tree.folders.map((f) => f.name)).toEqual(["Resumes", "Sessions"]);
    expect(tree.folders[1]?.notes.map((n) => n.entry)).toEqual(["map", "s11"]);
    expect(noteName("Sessions/Session 11.md")).toBe("Session 11");
  });

  it("adds folder entries, empty ones included, and skips deleted ones (RFC 0007)", () => {
    const folder = (path: string, deleted = false): EntryState => ({
      ...entry(path),
      blob: undefined,
      size: 0,
      folder: true,
      deleted,
    });
    const tree = buildTree(listed, [
      folder("PNJ"),
      folder("Sessions/Prep"),
      folder("Gone", true),
    ]);
    expect(tree.folders.map((f) => f.name)).toEqual([
      "PNJ",
      "Resumes",
      "Sessions",
    ]);
    const pnj = tree.folders[0];
    expect([pnj?.folders.length, pnj?.notes.length]).toEqual([0, 0]);
    expect(tree.folders[2]?.folders.map((f) => f.path)).toEqual([
      "Sessions/Prep/",
    ]);
    expect(notesInOrder(tree).map((n) => n.entry)).toEqual([
      "r12",
      "map",
      "s11",
    ]);
  });

  it("finds a folder's own note by the folder's name, Markdown only", () => {
    const tree = buildTree([
      entry("Golem/Golem.md", "g"),
      entry("Golem/Golem.png", "gp"),
      entry("Golem/Notes.md", "n"),
      entry("Runes/runes.md", "r"),
      entry("Assets/Assets.png", "a"),
    ]);
    const [assets, golem, runes] = tree.folders;
    expect(golem && folderNote(golem)?.entry).toBe("g");
    expect(runes && folderNote(runes)).toBeUndefined();
    expect(assets && folderNote(assets)).toBeUndefined();
  });

  it("orders notes as the tree shows them, and names the folders on a path", () => {
    const tree = buildTree([entry("z.md"), ...listed]);
    expect(notesInOrder(tree).map((n) => n.path)).toEqual([
      ...tree.folders.flatMap((f) => f.notes.map((n) => n.path)),
      "z.md",
    ]);
    expect(foldersOf("a/b/c.md")).toEqual(["a/", "a/b/"]);
    expect(foldersOf("c.md")).toEqual([]);
  });

  it("resolves a target by path or by name, and only among listed entries", () => {
    const find = resolver(listed);
    expect(find("Session 11")?.entry).toBe("s11");
    expect(find("sessions/session 11.md")?.entry).toBe("s11");
    expect(find("./Resumes/s12")?.entry).toBe("r12");
    expect(find("MJ/secret")).toBeUndefined();
    expect(find("")).toBeUndefined();
  });
});

describe("render", () => {
  // T6: the corpus test of RFC 0005 §14.
  it.each([
    "<script>alert(1)</script>",
    "<img src=x onerror=alert(1)>",
    '<a href="javascript:alert(1)">x</a>',
    "[x](javascript:alert(1))",
    "[x](data:text/html,<script>alert(1)</script>)",
    '<iframe src="https://evil.example"></iframe>',
    "<svg><script>alert(1)</script></svg>",
    '<div style="background:url(https://evil.example)">x</div>',
    "![x](https://evil.example/p.png)",
    "[[<img src=x onerror=alert(1)>]]",
  ])("leaves nothing that runs or calls out from %s", (md) => {
    const { body } = renderNote(md, listed);
    const doc = new DOMParser().parseFromString(body, "text/html");
    expect(
      doc.querySelectorAll("script, iframe, object, embed, style"),
    ).toHaveLength(0);
    for (const el of Array.from(doc.body.querySelectorAll("*"))) {
      for (const attr of Array.from(el.attributes)) {
        expect(attr.name, el.outerHTML).not.toMatch(/^on|^style$/i);
        // A link may leave, on a click (T7); nothing loads by itself.
        if (attr.name === "href")
          expect(attr.value, el.outerHTML).toMatch(/^(#|https?:\/\/|mailto:)/);
        expect(attr.name, el.outerHTML).not.toMatch(
          /^(src|srcset|poster|background)$/,
        );
      }
    }
  });

  it("links listed notes by entry id, and leaves hidden ones as plain text", () => {
    const { body } = renderNote(
      "See [[Session 11]], [[MJ/secret|the plan]] and [s12](Resumes/s12.md).",
      listed,
    );
    expect(body).toContain(`href="${ENTRY_LINK}s11"`);
    expect(body).toContain(`href="${ENTRY_LINK}r12"`);
    expect(body).toContain("the plan");
    expect(body).not.toContain("MJ/secret");
    expect(body).not.toMatch(/href="[^"]*plan/);
  });

  it("keeps external links, embeds listed images and drops remote ones", async () => {
    const r = renderNote(
      "[site](https://example.org) ![[map.png]] ![remote](https://example.org/p.png)",
      listed,
    );
    expect(r.body).toContain('href="https://example.org"');
    expect(r.images.map((e) => e.entry)).toEqual(["map"]);
    const doc = await frameDocument(
      r,
      async () => new Uint8Array([137, 80, 78, 71]),
    );
    expect(doc).toContain('src="data:image/png;base64,iVBORw=="');
    expect(doc).not.toContain("example.org/p.png");
    expect(doc).toContain(`<script>${BRIDGE}</script>`);
    expect(doc.match(/<script/g)).toHaveLength(1);
    const unreadable = await frameDocument(
      renderNote("![[map.png]]", listed),
      async () => null,
    );
    expect(unreadable).not.toContain("<img");
  });
});

describe("caps (T9)", () => {
  it("never fetches an image over MAX_IMAGE, and shows its name instead", async () => {
    const big = [entry("Sessions/huge.png", "huge", MAX_IMAGE + 1)];
    const load = vi.fn(async () => new Uint8Array([1]));
    const doc = await frameDocument(
      renderNote("![[huge.png|the map]]", big),
      load,
    );
    expect(load).not.toHaveBeenCalled();
    expect(doc).not.toContain("<img");
    expect(doc).toContain("the map");
    const atCap = [entry("Sessions/huge.png", "huge", MAX_IMAGE)];
    await frameDocument(renderNote("![[huge.png]]", atCap), load);
    expect(load).toHaveBeenCalledOnce();
  });
});

describe("time", () => {
  it("says how long ago, and a calendar day", () => {
    const now = Date.parse("2026-11-17T12:00:00Z");
    expect(ago("2026-11-17T10:00:00Z", "en", now)).toBe("2 hours ago");
    expect(ago("2026-11-17T11:59:50Z", "en", now)).toBe("now");
    expect(ago("2026-11-14T12:00:00Z", "fr", now)).toBe("il y a 3 jours");
    expect(day("2026-11-08T21:14:00Z", "en")).toContain("2026");
  });
});

describe("content", () => {
  it("parses chunk records and refuses a cut one", () => {
    const record = (byte: number, status: number, data: number[]) => [
      ...new Array(32).fill(byte),
      status,
      0,
      0,
      0,
      data.length,
      ...data,
    ];
    const body = new Uint8Array([
      ...record(1, 0, [104, 105]),
      ...record(2, 1, []),
    ]);
    const out = parseRecords(body);
    expect(
      new TextDecoder().decode(out.get("01".repeat(32)) ?? new Uint8Array()),
    ).toBe("hi");
    expect(out.get("02".repeat(32))).toBeNull();
    expect(() => parseRecords(body.subarray(0, 40))).toThrow(/cut short/);
  });
});

describe("the bridge", () => {
  it("has the hash the build writes", async () => {
    expect(await bridgeHash()).toMatch(/^'sha256-[A-Za-z0-9+/]{43}='$/);
  });
});

describe("note frame", () => {
  it("is sandboxed without same-origin, and trusts only its own frame's messages", async () => {
    const wrapper = mount(NoteFrame, {
      props: { document: "<p>x</p>", title: "Session 12" },
      attachTo: document.body,
    });
    const frame = wrapper.find("iframe");
    expect(frame.attributes("sandbox")).toBe("allow-scripts");
    expect(frame.attributes("srcdoc")).toBe("<p>x</p>");
    const vm = wrapper.vm as unknown as {
      onMessage: (e: MessageEvent) => void;
      height: number;
    };
    const own = (frame.element as HTMLIFrameElement).contentWindow;
    vm.onMessage(
      new MessageEvent("message", {
        data: { kind: "height", value: 640 },
        source: own,
      }),
    );
    vm.onMessage(
      new MessageEvent("message", {
        data: { kind: "height", value: 9 },
        source: window,
      }),
    );
    vm.onMessage(
      new MessageEvent("message", {
        data: { kind: "link", value: "#entry:s11" },
        source: own,
      }),
    );
    vm.onMessage(
      new MessageEvent("message", {
        data: { kind: "link", value: 42 },
        source: own,
      }),
    );
    vm.onMessage(new MessageEvent("message", { data: "noise", source: own }));
    await wrapper.vm.$nextTick();
    expect(vm.height).toBe(640);
    expect(wrapper.emitted("link")).toEqual([["#entry:s11"]]);
    // A theme switch reaches the frame as a message, not a reload.
    const post = vi.spyOn(own as Window, "postMessage");
    theme.value = theme.value === "dark" ? "light" : "dark";
    await wrapper.vm.$nextTick();
    expect(post).toHaveBeenCalledWith({ theme: theme.value }, "*");
    expect(frame.attributes("srcdoc")).toBe("<p>x</p>");
    wrapper.unmount();
  });
});
