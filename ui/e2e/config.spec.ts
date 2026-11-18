/**
 * RFC 0008 §6.P observations 1 and 2, in Chromium against the local stack:
 * alice's Configuration page lists her plugin by name, shows her snippet as
 * text, and never asks for the plugin's settings. Set by
 * tests/heavy/config-view.sh: KURA_E2E_CONFIG (the vault suite-config),
 * KURA_E2E_SNIPPET, KURA_E2E_PLUGIN, KURA_E2E_DATA_BLOB, KURA_E2E_AUTHENTIK.
 */
import { expect, test } from "@playwright/test";

import { env, signIn, signOut } from "./helpers";

test("alice reads her configuration, and nothing of a plugin's settings", async ({
  page,
}) => {
  const asked: string[] = [];
  page.on("request", (r) => asked.push(r.url()));
  await signIn(page, "alice");
  const vault = env("KURA_E2E_CONFIG");
  await page.goto(`/app/vaults/${vault}/config?profile=desktop`);
  const plugin = page
    .locator("[data-test=plugin]")
    .filter({ hasText: env("KURA_E2E_PLUGIN") });
  await expect(plugin).toContainText("1.0.0");
  await expect(plugin).toContainText("settings, not shown");
  await expect(plugin.locator("a", { hasText: "data.json" })).toHaveCount(0);

  const snippet = env("KURA_E2E_SNIPPET");
  await page.locator(`[data-file="${snippet}"]`).click();
  await expect(page.locator("[data-test=text]")).toContainText("color: red");
  await expect(page.locator("[data-test=file]")).toContainText("Last edited");
  await page.locator("[data-test=back]").click();
  await expect(page.locator(`[data-file="${snippet}"]`)).toBeFocused();

  const data = env("KURA_E2E_DATA_BLOB");
  expect(asked.filter((u) => u.includes(data))).toEqual([]);
  await signOut(page);
});
