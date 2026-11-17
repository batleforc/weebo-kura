/**
 * RFC 0003 §6.P observation 4, the page half: alice creates a rule from the
 * console, sees its secret once and clicks Test. tests/heavy/notify.sh reads
 * the secret from KURA_E2E_SECRET_FILE and checks the receiver's log.
 */
import { writeFileSync } from "node:fs";

import { expect, test } from "@playwright/test";

import { env, signIn, signOut } from "./helpers";

test("4: alice creates a rule and tests it", async ({ page }) => {
  await signIn(page, "alice");
  await page.goto("/app/manage/tenants/dev/notifications");
  await page.locator("[data-test=new-rule]").click();
  await page.locator("[data-test=name]").fill(`hub-${Date.now()}`);
  await page.locator("input[value='device.revoked']").check();
  await page.locator("[data-test=url]").fill(env("KURA_E2E_HOOK"));
  await page.locator("[data-test=authorization]").fill("Bearer test-key");
  await page.locator("[data-test=create]").click();
  const secret = (await page.locator("[data-test=secret]").textContent()) ?? "";
  expect(secret).toMatch(/^whsec_/);
  writeFileSync(env("KURA_E2E_SECRET_FILE"), secret);
  await page.locator("[data-test=close]").click();
  await page.locator("tbody tr").last().locator("[data-test=test]").click();
  await expect(page.locator("[data-test=tested]")).toContainText(": 200");
  await signOut(page);
});
