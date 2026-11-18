/**
 * RFC 0006 §6.P observation 7, the page half: alice, who owns the tenant,
 * opens the vault `notes` and finds bob's device in its Devices section,
 * named by its owner's name, with where it stands and its last run.
 * tests/heavy/devices.sh sets the device it expects.
 */
import { expect, test } from "@playwright/test";

import { env, signIn, signOut } from "./helpers";

test("7: alice sees where bob's device stands on notes", async ({ page }) => {
  await signIn(page, "alice");
  await page.goto(`/app/vaults/${env("KURA_E2E_NOTES")}`);
  await expect(
    page.getByRole("heading", { name: "Devices syncing it" }),
  ).toBeVisible();
  const row = page.locator(
    `[data-test=sync] tr[data-device="${env("KURA_E2E_DEVICE")}"]`,
  );
  await expect(row).toContainText("up to date");
  await expect(row).toContainText(env("KURA_E2E_OWNER"));
  await expect(row.locator("[data-test=last-run]")).toContainText(
    "1 file cannot sync",
  );
  await signOut(page);
});
