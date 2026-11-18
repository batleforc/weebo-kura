import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";

import type { EntryState } from "@/api/generated/types.gen";
import { i18n } from "@/i18n";
import FolderTree from "./FolderTree.vue";
import { buildTree } from "./tree";

const entry = (path: string, id: string): EntryState => ({
  entry: id,
  path,
  revision: `r-${id}`,
  seq: 1,
  size: 1,
  deleted: false,
  blob: `b-${id}`,
});

describe("folder tree", () => {
  it("opens a folder's own note from the folder's name; other folders only fold", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/:rest(.*)", component: { template: "<p/>" } }],
    });
    const tree = buildTree([
      entry("Golem/Golem.md", "g"),
      entry("Golem/Iron.md", "i"),
      entry("Runes/a.md", "a"),
    ]);
    const w = mount(FolderTree, {
      props: { folder: tree, vault: "v1", open: new Set<string>() },
      global: { plugins: [router, i18n] },
    });
    const links = w.findAll("[data-test=folder-note]");
    expect(links.map((l) => [l.text(), l.attributes("href")])).toEqual([
      ["Golem", "/vaults/v1/notes/g"],
    ]);
    await links[0]?.trigger("click");
    await router.isReady();
    await new Promise((r) => setTimeout(r));
    expect(router.currentRoute.value.path).toBe("/vaults/v1/notes/g");
    // Runes has no note of its own: its name stays plain text.
    expect(w.find("[data-folder='Runes/'] summary").find("a").exists()).toBe(
      false,
    );
  });
});
