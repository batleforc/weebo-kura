/** Shared test helpers: a fake identity provider and storage reset. */

import { flushPromises, mount } from "@vue/test-utils";
import { AxiosError, AxiosHeaders } from "axios";
import { vi } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";

import { forgetDiscovery } from "@/auth/discovery";
import { setConfig } from "@/config";
import { i18n } from "@/i18n";

export const IDP = "https://auth.example.org";

export const metadata = {
  issuer: `${IDP}/`,
  authorization_endpoint: `${IDP}/application/o/authorize/`,
  token_endpoint: `${IDP}/application/o/token/`,
  revocation_endpoint: `${IDP}/application/o/revoke/`,
  code_challenge_methods_supported: ["S256", "plain"],
};

export function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

/** Resets storage and configuration; returns the fetch mock, answering discovery. */
export function freshIdp(
  token?: (body: URLSearchParams) => Response | Promise<Response>,
) {
  localStorage.clear();
  sessionStorage.clear();
  forgetDiscovery();
  setConfig({
    issuer: `${IDP}/`,
    discovery: `${IDP}/application/o/kura-web/`,
    clientId: "kura-web",
  });
  const fetchMock = vi.fn(
    async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (url.endsWith("/.well-known/openid-configuration"))
        return json(metadata);
      if (url === metadata.token_endpoint && token)
        return token(new URLSearchParams(String(init?.body)));
      if (url === metadata.revocation_endpoint)
        return new Response(null, { status: 200 });
      return new Response("unexpected", { status: 500 });
    },
  );
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

/** An API refusal as axios throws it: `/v1/problems/<type>`. */
export function refusal(
  status: number,
  type?: string,
  detail?: string,
): AxiosError {
  const config = { headers: new AxiosHeaders() };
  return new AxiosError("refused", "ERR", config, null, {
    status,
    statusText: "",
    headers: {},
    config,
    data: type ? { type: `/v1/problems/${type}`, detail } : {},
  });
}

/** Mounts a page at a path, any other route an empty stub; returns the wrapper. */
export async function at(component: object, path: string, routePath: string) {
  const router = createRouter({
    history: createMemoryHistory("/app/"),
    routes: [
      { path: routePath, component },
      { path: "/:any(.*)*", component: { template: "<p/>" } },
    ],
  });
  await router.push(path);
  const wrapper = mount(component, { global: { plugins: [router, i18n] } });
  await flushPromises();
  await flushPromises();
  return wrapper;
}
