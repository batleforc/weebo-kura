/** Shared by the browser proofs: environment and the real Authentik sign-in. */
import type { Page } from "@playwright/test";

export const env = (name: string): string => {
  const value = process.env[name];
  if (!value)
    throw new Error(
      `${name} is not set: run the tests/heavy suite that drives this file`,
    );
  return value;
};

/** Through the console's button and Authentik's own pages. */
export async function signIn(page: Page, user: string): Promise<void> {
  await page.goto("/app/signin");
  await page.getByRole("button", { name: /Authentik/ }).click();
  await page.waitForURL(new RegExp(`^${env("KURA_E2E_AUTHENTIK")}`));
  // Authentik's own stages, by their accessible names: each appears after
  // the previous one is answered.
  await page.getByRole("textbox", { name: /Username|Email/i }).fill(user);
  await page
    .getByRole("button", { name: /Log in|Continue/i })
    .first()
    .click();
  const password = page.getByRole("textbox", { name: "Password" });
  await password.waitFor();
  await password.fill("kura-dev");
  await page.getByRole("button", { name: "Continue" }).click();
  // The consent stage, the first time this user meets the console.
  const consent = page
    .locator(
      'ak-stage-consent button[type="submit"], button:has-text("Continue")',
    )
    .first();
  await Promise.race([
    page.waitForURL(/\/app\/vaults/, { timeout: 20_000 }),
    consent
      .click({ timeout: 20_000 })
      .then(() => page.waitForURL(/\/app\/vaults/, { timeout: 20_000 })),
  ]);
}
