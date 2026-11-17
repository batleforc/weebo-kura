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
    component: () => import("@/pages/VaultsPage.vue"),
    meta: { access: "member" },
  },
  {
    path: "/devices",
    name: "devices",
    component: () => import("@/pages/DevicesPage.vue"),
    meta: { access: "member" },
  },
  // The reader (RFC 0005): browse, read, search, history.
  {
    path: "/vaults/:v",
    name: "browse",
    component: () => import("@/pages/BrowsePage.vue"),
    meta: { access: "member" },
  },
  {
    path: "/vaults/:v/notes/:entry",
    name: "note",
    component: () => import("@/pages/BrowsePage.vue"),
    meta: { access: "member" },
  },
  {
    path: "/vaults/:v/notes/:entry/history",
    name: "history",
    component: () => import("@/pages/HistoryPage.vue"),
    meta: { access: "member" },
  },
  {
    path: "/vaults/:v/search",
    name: "search",
    component: () => import("@/pages/SearchPage.vue"),
    meta: { access: "member" },
  },
  { path: "/:pathMatch(.*)*", redirect: "/vaults" },
];

export function makeRouter(history = createWebHistory("/app/")): Router {
  const router = createRouter({ history, routes });
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
