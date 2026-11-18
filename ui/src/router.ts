/**
 * Every page, and who may open it. The guard refuses a route that does not
 * declare `access` (after proxy-cache's router, R31): a page added without
 * thinking about it is not reachable.
 */

import {
  createRouter,
  createWebHistory,
  type RouteRecordRaw,
  type Router,
} from "vue-router";

import { useSession } from "@/auth/session";

declare module "vue-router" {
  interface RouteMeta {
    /** `public`: anyone; `member`: a signed-in caller. */
    access?: "public" | "member";
  }
}

export const routes: RouteRecordRaw[] = [
  { path: "/", redirect: "/vaults" },
  {
    path: "/signin",
    name: "signin",
    component: () => import("@/pages/SignInPage.vue"),
    meta: { access: "public" },
  },
  {
    path: "/callback",
    name: "callback",
    component: () => import("@/pages/CallbackPage.vue"),
    meta: { access: "public" },
  },
  {
    path: "/vaults",
    name: "vaults",
    component: () => import("@/pages/HomePage.vue"),
    meta: { access: "member" },
  },
  {
    path: "/devices",
    name: "devices",
    component: () => import("@/pages/DevicesPage.vue"),
    meta: { access: "member" },
  },
  {
    path: "/help",
    name: "help",
    component: () => import("@/pages/HelpPage.vue"),
    meta: { access: "member" },
  },
  // One vault: its overview, the reader (RFC 0005) and, for its managers,
  // the back office of RFC 0003, as tabs; the API decides, the tabs only show.
  {
    path: "/vaults/:v",
    component: () => import("@/pages/VaultLayout.vue"),
    meta: { access: "member" },
    children: [
      {
        path: "",
        name: "vault",
        component: () => import("@/pages/VaultOverview.vue"),
        meta: { access: "member" },
      },
      {
        path: "notes",
        name: "vault-notes",
        component: () => import("@/pages/BrowsePage.vue"),
        meta: { access: "member" },
      },
      {
        path: "notes/:entry",
        name: "note",
        component: () => import("@/pages/BrowsePage.vue"),
        meta: { access: "member" },
      },
      {
        path: "notes/:entry/history",
        name: "history",
        // The note again, its history open in a drawer beside it.
        component: () => import("@/pages/BrowsePage.vue"),
        meta: { access: "member" },
      },
      {
        path: "search",
        name: "search",
        component: () => import("@/pages/SearchPage.vue"),
        meta: { access: "member" },
      },
      {
        path: "config",
        name: "vault-config",
        component: () => import("@/pages/ConfigPage.vue"),
        meta: { access: "member" },
      },
      {
        path: "access",
        name: "vault-access",
        component: () => import("@/pages/AccessPage.vue"),
        meta: { access: "member" },
      },
      {
        path: "keys",
        name: "vault-keys",
        component: () => import("@/pages/KeysPage.vue"),
        meta: { access: "member" },
      },
      {
        path: "notifications",
        name: "vault-notifications",
        component: () => import("@/pages/NotificationsPage.vue"),
        meta: { access: "member" },
      },
    ],
  },
  // The tenant-wide back office, reached from the home page; the old
  // per-vault addresses lead to the vault's tabs.
  { path: "/manage", redirect: "/vaults" },
  {
    path: "/manage/vaults/:v/access",
    redirect: (to) => `/vaults/${String(to.params.v)}/access`,
  },
  {
    path: "/manage/vaults/:v/keys",
    redirect: (to) => `/vaults/${String(to.params.v)}/keys`,
  },
  {
    path: "/manage/tenants/:t/vaults",
    name: "tenant-vaults",
    component: () => import("@/pages/TenantVaultsPage.vue"),
    meta: { access: "member" },
  },
  {
    path: "/manage/devices",
    name: "tenant-devices",
    component: () => import("@/pages/TenantDevicesPage.vue"),
    meta: { access: "member" },
  },
  {
    path: "/manage/status",
    name: "status",
    component: () => import("@/pages/StatusPage.vue"),
    meta: { access: "member" },
  },
  {
    path: "/manage/tenants/:t/notifications",
    name: "notifications",
    component: () => import("@/pages/NotificationsPage.vue"),
    meta: { access: "member" },
  },
  {
    path: "/manage/tenants/:t/notifications/:n",
    name: "deliveries",
    component: () => import("@/pages/DeliveriesPage.vue"),
    meta: { access: "member" },
  },
  { path: "/:pathMatch(.*)*", redirect: "/vaults" },
];

export function makeRouter(history = createWebHistory("/app/")): Router {
  const router = createRouter({
    history,
    routes,
    // shortcut: a saved place is restored before an async note has its height, so a long note may land short; restore after the frame reports its height if it shows.
    scrollBehavior(to, from, saved) {
      if (saved) return saved;
      // Opening or closing a note's history keeps the reader's place.
      if (to.params.entry && to.params.entry === from.params.entry)
        return false;
      return to.path === from.path ? false : { top: 0 };
    },
  });
  router.beforeEach(async (to) => {
    const access = to.meta.access;
    if (access === "public") return true;
    if (access !== "member") return false;
    const session = useSession();
    if (session.authenticated || (await session.restore())) return true;
    return { name: "signin", query: { to: to.fullPath } };
  });
  return router;
}
