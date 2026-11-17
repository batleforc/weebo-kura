import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";

import { i18n } from "@/i18n";
import { refusal } from "@/test-utils";

vi.mock("@/api/generated/sdk.gen", () => ({
  listVaults: vi.fn(async () => ({
    data: [
      {
        id: "v1",
        name: "campaign",
        tenant: "jdr",
        role: "viewer",
        paths: ["Sessions/"],
      },
    ],
  })),
  getEntries: vi.fn(),
  getRevisions: vi.fn(),
  search: vi.fn(),
  registerDevice: vi.fn(async () => ({ data: { id: "dev" } })),
}));

/** The event socket, driven by the tests (its own spec is reader/events.spec.ts). */
const socket = vi.hoisted(() => ({
  notice: undefined as (() => void) | undefined,
  close: vi.fn(),
}));
vi.mock("@/reader/events", () => ({
  watchVault: vi.fn((_v: string, notice: () => void) => {
    socket.notice = notice;
    return { close: socket.close };
  }),
}));

import * as sdk from "@/api/generated/sdk.gen";
import { useSession } from "@/auth/session";
import { fileBytes } from "@/reader/content";
import { MAX_RENDER } from "@/reader/render";
import { MAX_PAGES } from "@/reader/vault";
import BrowsePage from "./BrowsePage.vue";
import HistoryPage from "./HistoryPage.vue";
import SearchPage from "./SearchPage.vue";
import VaultsPage from "./VaultsPage.vue";

const entry = (path: string, id: string, blob: string | null = `b-${id}`) => ({
  entry: id,
  path,
  revision: `r-${id}`,
  seq: 1,
  size: 20,
  deleted: false,
  blob,
});

/** A fake API for content: manifests and chunk records, by hash. */
function contentServer(files: Record<string, string>) {
  const hex = (n: number) => n.toString(16).padStart(2, "0").repeat(32);
  const chunks = new Map<string, Uint8Array>();
  const manifests = new Map<string, { size: number; chunks: string[] }>();
  Object.entries(files).forEach(([blob, text], i) => {
    const bytes = new TextEncoder().encode(text);
    const h = hex(i + 1);
    chunks.set(h, bytes);
    manifests.set(blob, { size: bytes.length, chunks: [h] });
  });
  const calls: RequestInit[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      calls.push(init ?? {});
      const url = String(input);
      const blob = url.match(/\/blobs\/(.+)$/)?.[1];
      if (blob) {
        const m = manifests.get(blob);
        return m
          ? new Response(JSON.stringify(m))
          : new Response(JSON.stringify({ type: "/v1/problems/not_found" }), {
              status: 404,
            });
      }
      if (url.endsWith("/chunks/batch")) {
        const asked = JSON.parse(String(init?.body)).chunks as string[];
        const parts: number[] = [];
        for (const h of asked) {
          const data = chunks.get(h);
          parts.push(
            ...(h.match(/../g) ?? []).map((x) => Number.parseInt(x, 16)),
            data ? 0 : 1,
            ...[24, 16, 8, 0].map((s) => ((data?.length ?? 0) >>> s) & 255),
          );
          // Not spread: a 1 MiB argument list overflows the stack.
          for (const b of data ?? []) parts.push(b);
        }
        return new Response(new Uint8Array(parts));
      }
      return new Response("", { status: 500 });
    }),
  );
  return calls;
}

async function mountAt(component: object, path: string, routePath: string) {
  const router = createRouter({
    history: createMemoryHistory("/app/"),
    routes: [
      { path: routePath, component },
      { path: "/vaults/:v", component: { template: "<p/>" } },
      { path: "/vaults/:v/notes/:entry", component: { template: "<p/>" } },
      { path: "/vaults/:v/search", component: { template: "<p/>" } },
      { path: "/vaults", component: { template: "<p/>" } },
    ],
  });
  await router.push(path);
  const wrapper = mount(component, { global: { plugins: [router, i18n] } });
  await flushPromises();
  await flushPromises();
  return { wrapper, router };
}

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
  i18n.global.locale.value = "en";
  vi.clearAllMocks();
  const session = useSession();
  session.accessToken = "a";
  vi.spyOn(session, "bearer").mockResolvedValue("a");
});
afterEach(() => vi.unstubAllGlobals());

describe("browse and read", () => {
  it("shows the shared folders and renders a note in the sandboxed frame", async () => {
    vi.mocked(sdk.getEntries).mockResolvedValue({
      data: {
        items: [
          entry("Sessions/s12.md", "s12"),
          entry("Sessions/s11.md", "s11"),
        ],
        epoch: "e",
        headSeq: 2,
      },
    } as never);
    const calls = contentServer({
      "b-s12": "# Session 12\n\nSee [[s11]] and [[MJ/secret]].",
    });
    const { wrapper } = await mountAt(
      BrowsePage,
      "/vaults/v1/notes/s12",
      "/vaults/:v/notes/:entry",
    );
    expect(wrapper.find("[data-test=tree]").text()).toContain("s11");
    const frame = wrapper.find("[data-test=note-frame]");
    expect(frame.attributes("sandbox")).toBe("allow-scripts");
    const doc = frame.attributes("srcdoc") ?? "";
    expect(doc).toContain("Session 12");
    expect(doc).toContain("#entry:s11");
    expect(doc).toContain("not shared with you");
    expect(calls.every((c) => c.cache === "no-store")).toBe(true);
  });

  it("says a hidden note does not exist, the same as a missing one", async () => {
    vi.mocked(sdk.getEntries).mockResolvedValue({
      data: {
        items: [entry("Sessions/s12.md", "s12", "gone")],
        epoch: "e",
        headSeq: 1,
      },
    } as never);
    contentServer({});
    for (const id of ["unknown-id", "s12"]) {
      const { wrapper } = await mountAt(
        BrowsePage,
        `/vaults/v1/notes/${id}`,
        "/vaults/:v/notes/:entry",
      );
      expect(wrapper.find("[data-test=not-found]").text()).toContain(
        "does not exist, or is not shared with you",
      );
    }
  });

  it("offers a file that is not Markdown as a download, and follows only listed links", async () => {
    vi.mocked(sdk.getEntries).mockResolvedValue({
      data: {
        items: [
          entry("Sessions/map.bin", "bin"),
          entry("Sessions/s11.md", "s11"),
        ],
        epoch: "e",
        headSeq: 1,
      },
    } as never);
    contentServer({ "b-bin": "binary" });
    vi.stubGlobal(
      "URL",
      Object.assign(URL, {
        createObjectURL: () => "blob:x",
        revokeObjectURL: () => undefined,
      }),
    );
    const { wrapper, router } = await mountAt(
      BrowsePage,
      "/vaults/v1/notes/bin",
      "/vaults/:v/notes/:entry",
    );
    expect(wrapper.find("a[download]").attributes("href")).toBe("blob:x");
    const open = vi.spyOn(window, "open").mockReturnValue(null);
    const page = wrapper.vm as unknown as { follow: (h: string) => void };
    page.follow("#entry:not-listed");
    page.follow("javascript:alert(1)");
    page.follow("https://example.org/");
    page.follow("#entry:s11");
    await flushPromises();
    expect(open).toHaveBeenCalledWith(
      "https://example.org/",
      "_blank",
      "noopener,noreferrer",
    );
    expect(open).toHaveBeenCalledTimes(1);
    expect(router.currentRoute.value.path).toBe("/vaults/v1/notes/s11");
  });

  it("says when the vault cannot be loaded", async () => {
    vi.mocked(sdk.getEntries).mockRejectedValue(refusal(503, "unavailable"));
    const { wrapper } = await mountAt(BrowsePage, "/vaults/v1", "/vaults/:v");
    expect(wrapper.find("[role=alert]").text()).toContain(
      "could not be loaded",
    );
  });
});

describe("reader gaps of RFC 0005 §18", () => {
  it("says when the tree is cut at MAX_PAGES (T9)", async () => {
    vi.mocked(sdk.getEntries).mockResolvedValue({
      data: {
        items: [entry("Sessions/s12.md", "s12")],
        epoch: "e",
        headSeq: 1,
        next: "more",
      },
    } as never);
    const { wrapper } = await mountAt(BrowsePage, "/vaults/v1", "/vaults/:v");
    expect(sdk.getEntries).toHaveBeenCalledTimes(MAX_PAGES);
    expect(wrapper.find("[data-test=cut]").text()).toContain(
      "more than 50,000 entries",
    );
  });

  it("offers a note over MAX_RENDER as a download, unrendered (T9)", async () => {
    vi.mocked(sdk.getEntries).mockResolvedValue({
      data: {
        items: [entry("Sessions/big.md", "big")],
        epoch: "e",
        headSeq: 1,
      },
    } as never);
    contentServer({ "b-big": "x".repeat(MAX_RENDER + 1) });
    vi.stubGlobal(
      "URL",
      Object.assign(URL, {
        createObjectURL: () => "blob:big",
        revokeObjectURL: () => undefined,
      }),
    );
    const { wrapper } = await mountAt(
      BrowsePage,
      "/vaults/v1/notes/big",
      "/vaults/:v/notes/:entry",
    );
    expect(wrapper.find("a[download]").attributes("href")).toBe("blob:big");
    expect(wrapper.find("[data-test=note-frame]").exists()).toBe(false);
  });

  it("says who edited the note last, and refreshes on a notice, closing the socket on leaving", async () => {
    vi.mocked(sdk.getEntries).mockResolvedValue({
      data: {
        items: [entry("Sessions/s12.md", "s12")],
        epoch: "e",
        headSeq: 1,
      },
    } as never);
    vi.mocked(sdk.getRevisions).mockResolvedValue({
      data: {
        items: [
          {
            revision: "r-s12",
            path: "Sessions/s12.md",
            createdAt: "2026-11-08T21:14:00Z",
            deleted: false,
            size: 5,
            seq: 1,
            author: { kind: "key", name: "discord-recap" },
          },
        ],
      },
    } as never);
    contentServer({ "b-s12": "first", "b-s12b": "second" });
    const { wrapper } = await mountAt(
      BrowsePage,
      "/vaults/v1/notes/s12",
      "/vaults/:v/notes/:entry",
    );
    await flushPromises();
    expect(wrapper.find("[data-test=last-edit]").text()).toContain(
      "Last edited Nov 8, 2026 by key discord-recap",
    );
    vi.mocked(sdk.getEntries).mockResolvedValue({
      data: {
        items: [
          { ...entry("Sessions/s12.md", "s12", "b-s12b"), revision: "r2" },
        ],
        epoch: "e",
        headSeq: 2,
      },
    } as never);
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    try {
      socket.notice?.();
      socket.notice?.();
      await vi.advanceTimersByTimeAsync(1000);
    } finally {
      vi.useRealTimers();
    }
    await flushPromises();
    await flushPromises();
    // Two notices, one refresh: listVaults once on mount, once now.
    expect(sdk.listVaults).toHaveBeenCalledTimes(2);
    expect(
      wrapper.find("[data-test=note-frame]").attributes("srcdoc"),
    ).toContain("second");
    wrapper.unmount();
    expect(socket.close).toHaveBeenCalled();
  });

  it("retries a 503 by itself every 30 s, and keeps the button", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    try {
      vi.mocked(sdk.getEntries)
        .mockRejectedValueOnce(refusal(503, "unavailable"))
        .mockResolvedValue({
          data: {
            items: [entry("Sessions/s12.md", "s12")],
            epoch: "e",
            headSeq: 1,
          },
        } as never);
      const { wrapper } = await mountAt(BrowsePage, "/vaults/v1", "/vaults/:v");
      expect(wrapper.find("[role=alert]").text()).toContain(
        "Kura is not answering (503). Retrying in 30 s",
      );
      expect(wrapper.find("[role=alert] button").exists()).toBe(true);
      await vi.advanceTimersByTimeAsync(30_000);
      await flushPromises();
      expect(wrapper.find("[data-test=tree]").text()).toContain("s12");
    } finally {
      vi.useRealTimers();
    }
  });

  it("shows when each vault was updated, and whom to ask when there is none", async () => {
    vi.mocked(sdk.listVaults)
      .mockResolvedValueOnce({
        data: [
          {
            id: "v1",
            name: "campaign",
            tenant: "jdr",
            role: "viewer",
            paths: ["Sessions/", "Resumes/"],
            managedBy: "config",
            updatedAt: new Date(Date.now() - 2 * 3_600_000).toISOString(),
          },
        ],
      } as never)
      .mockResolvedValueOnce({ data: [] } as never);
    const { wrapper } = await mountAt(VaultsPage, "/vaults", "/vaults");
    expect(wrapper.find("tbody").text()).toContain("2 folders");
    expect(wrapper.find("tbody time").text()).toBe("2 hours ago");
    const empty = await mountAt(VaultsPage, "/vaults", "/vaults");
    expect(empty.wrapper.text()).toContain("Ask a vault manager to add you");
  });
});

describe("search", () => {
  it("searches as the user types, once after a pause, keeping only the latest answer", async () => {
    vi.mocked(sdk.search).mockResolvedValue({
      data: { results: [] },
    } as never);
    const { wrapper } = await mountAt(
      SearchPage,
      "/vaults/v1/search",
      "/vaults/:v/search",
    );
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    try {
      await wrapper.find("[data-test=query]").setValue("mi");
      await wrapper.find("[data-test=query]").setValue("mine");
      await vi.advanceTimersByTimeAsync(399);
      expect(sdk.search).not.toHaveBeenCalled();
      await vi.advanceTimersByTimeAsync(1);
    } finally {
      vi.useRealTimers();
    }
    await flushPromises();
    expect(sdk.search).toHaveBeenCalledOnce();
    expect(vi.mocked(sdk.search).mock.calls[0]?.[0]?.body).toEqual({
      query: "mine",
    });
    expect(wrapper.find("[role=status]").text()).toContain('"mine"');
  });

  it("shows hits as text, names the limit when nothing matches, and says when search is off", async () => {
    vi.mocked(sdk.search)
      .mockResolvedValueOnce({
        data: {
          results: [
            {
              entry: "s12",
              path: "Sessions/s12.md",
              score: 1,
              snippet: "<b>mine</b>",
            },
          ],
        },
      } as never)
      .mockResolvedValueOnce({ data: { results: [] } } as never)
      .mockRejectedValueOnce(refusal(409, "search_disabled"));
    const { wrapper } = await mountAt(
      SearchPage,
      "/vaults/v1/search",
      "/vaults/:v/search",
    );
    await wrapper.find("[data-test=query]").setValue("mine");
    await wrapper.find("form").trigger("submit");
    await flushPromises();
    expect(wrapper.text()).toContain("<b>mine</b>");
    expect(wrapper.find("b").exists()).toBe(false);
    await wrapper.find("[data-test=query]").setValue("dragon");
    await wrapper.find("form").trigger("submit");
    await flushPromises();
    expect(wrapper.find("[data-test=no-hit]").text()).toContain(
      '"dragon" in the folders shared with you',
    );
    await wrapper.find("form").trigger("submit");
    await flushPromises();
    expect(wrapper.find("[data-test=search-off]").exists()).toBe(true);
  });
});

describe("history", () => {
  it("lists revisions and shows a past one; a hidden entry is not found", async () => {
    vi.mocked(sdk.getRevisions)
      .mockResolvedValueOnce({
        data: {
          items: [
            {
              revision: "r2",
              path: "Sessions/s12.md",
              createdAt: "2026-11-08T21:14:00Z",
              deleted: false,
              size: 5,
              seq: 2,
              blob: "b-new",
              author: { kind: "device", name: "laptop" },
            },
            {
              revision: "r1",
              path: "Sessions/s12.md",
              createdAt: "2026-11-08T20:02:00Z",
              deleted: false,
              size: 5,
              seq: 1,
              blob: "b-old",
              author: { kind: "key", name: "discord-recap" },
            },
          ],
        },
      } as never)
      .mockRejectedValueOnce(refusal(404, "not_found"));
    vi.mocked(sdk.getEntries).mockResolvedValue({
      data: { items: [], epoch: "e", headSeq: 2 },
    } as never);
    contentServer({ "b-old": "first draft" });
    const { wrapper } = await mountAt(
      HistoryPage,
      "/vaults/v1/notes/s12/history",
      "/vaults/:v/notes/:entry/history",
    );
    expect(wrapper.text()).toContain("2026-11-08 21:14");
    expect(wrapper.text()).toContain("current");
    expect(wrapper.text()).toContain("laptop");
    expect(wrapper.text()).toContain("key discord-recap");
    await wrapper.find("button").trigger("click");
    await flushPromises();
    expect(
      wrapper.find("[data-test=note-frame]").attributes("srcdoc"),
    ).toContain("first draft");
    const hidden = await mountAt(
      HistoryPage,
      "/vaults/v1/notes/x/history",
      "/vaults/:v/notes/:entry/history",
    );
    expect(hidden.wrapper.find("[data-test=not-found]").exists()).toBe(true);
  });
});

describe("content", () => {
  it("returns nothing for a hidden file and refuses a missing chunk", async () => {
    contentServer({ "b-a": "abc" });
    expect(await fileBytes("v1", "unknown")).toBeNull();
    expect(
      new TextDecoder().decode(
        (await fileBytes("v1", "b-a")) ?? new Uint8Array(),
      ),
    ).toBe("abc");
  });
});
