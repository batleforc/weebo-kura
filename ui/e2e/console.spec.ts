/**
 * RFC 0003 §6.P observations 1 to 3, in Chromium against the local stack:
 * the real Authentik sign-in, no cookie from Kura, a key's secret shown once,
 * and the refusal a viewer meets on a manager's page. Set by
 * tests/heavy/console.sh: KURA_E2E_NOTES (alice manages it), KURA_E2E_CAMPAIGN
 * (bob is only a viewer there), KURA_E2E_AUTHENTIK.
 */
import { expect, type Page, test } from "@playwright/test";

const env = (name: string): string => {
  const value = process.env[name];
  if (!value) throw new Error(`${name} is not set: run tests/heavy/console.sh`);
  return value;
};

/** Through the console's button and Authentik's own pages. */
async function signIn(page: Page, user: string): Promise<void> {
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

test("1 and 2: alice signs in, gets no cookie, and a key's secret is shown once", async ({
  page,
}) => {
  const api: {
    call: string;
    headers: Record<string, string>;
    body: string;
    setCookie?: string;
  }[] = [];
  page.on("response", async (r) => {
    const url = new URL(r.url());
    // Kura's answers only: Authentik's own pages (/application/…) set its cookie.
    const kura = new URL(page.url() || "http://127.0.0.1:58080").origin;
    if (url.origin !== new URL(process.env.KURA_DEV_SERVER ?? kura).origin)
      return;
    const headers = r.request().headers();
    api.push({
      call: `${r.request().method()} ${url.pathname}`,
      headers: url.pathname.startsWith("/v1/") ? headers : {},
      body: url.pathname.startsWith("/v1/")
        ? await r.text().catch(() => "")
        : "",
      setCookie: (await r.allHeaders())["set-cookie"],
    });
  });

  // 1. Through Authentik, onto My vaults; every /v1 call carries the token
  // and the device, and nothing from Kura sets a cookie.
  await signIn(page, "alice");
  await expect(page.getByRole("heading", { name: "My vaults" })).toBeVisible();
  await expect(page.locator("tbody")).toContainText("notes");
  const v1 = api.filter((a) => Object.keys(a.headers).length > 0);
  expect(v1.length).toBeGreaterThan(0);
  for (const a of v1) expect(a.headers.authorization).toMatch(/^Bearer /);
  // Without a device: who am I, and the registration, once.
  expect(
    v1.filter((a) => !a.headers["x-kura-device"]).map((a) => a.call),
  ).toEqual(["GET /v1/me", "POST /v1/devices"]);
  expect(api.filter((a) => a.setCookie)).toEqual([]);

  // 2. A key: its secret in the dialog, then nowhere after a reload.
  await page.goto(`/app/manage/vaults/${env("KURA_E2E_NOTES")}/keys`);
  await page.locator("[data-test=new-key]").click();
  await page.locator("[data-test=cidrs]").fill("127.0.0.0/8");
  await page.locator("[data-test=create]").click();
  const secret = (await page.locator("[data-test=secret]").textContent()) ?? "";
  expect(secret).toMatch(/^kura_/);
  await page.locator("[data-test=close]").click();
  api.length = 0;
  await page.reload();
  await expect(page.locator("tbody")).toContainText("127.0.0.0/8");
  expect(await page.content()).not.toContain(secret);
  expect(api.filter((a) => a.body.includes(secret))).toEqual([]);
});

test("3: bob, a viewer, is refused the keys of a vault, and is shown no key", async ({
  page,
}) => {
  const refused: { status: number; body: string }[] = [];
  page.on("response", async (r) => {
    if (/\/v1\/vaults\/[^/]+\/tokens$/.test(new URL(r.url()).pathname)) {
      refused.push({
        status: r.status(),
        body: await r.text().catch(() => ""),
      });
    }
  });
  await signIn(page, "bob");
  await page.goto(`/app/manage/vaults/${env("KURA_E2E_CAMPAIGN")}/keys`);
  await expect(page.locator("[data-test=forbidden]")).toHaveText(
    "Your role does not allow this page.",
  );
  expect(refused.map((r) => r.status)).toEqual([403]);
  expect(refused[0]?.body).toContain("/v1/problems/insufficient_role");
  expect(refused[0]?.body).not.toContain("kura_");
  await expect(page.locator("[data-test=manage]")).toHaveCount(0);
});
