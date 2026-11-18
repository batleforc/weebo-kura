/**
 * RFC 0005 §6.P observations 1, 5 (the page half) and 6, in Chromium. Set by
 * tests/heavy/reader.sh: KURA_E2E_REFRESH (bob's kura-web refresh token),
 * KURA_E2E_VAULT, KURA_E2E_NOTE (Sessions/s12.md), KURA_E2E_HIDDEN (the
 * entry of MJ/secret.md).
 */
import { expect, test } from "@playwright/test";

import { signOut } from "./helpers";

const env = (name: string): string => {
  const value = process.env[name];
  if (!value) throw new Error(`${name} is not set: run tests/heavy/reader.sh`);
  return value;
};

test("a restricted viewer reads the shared folders, and a hostile note runs nothing", async ({
  page,
}) => {
  const vault = env("KURA_E2E_VAULT");
  const outside: string[] = [];
  const violations: string[] = [];
  page.on("request", (r) => {
    const url = new URL(r.url());
    if (url.pathname === "/x" || url.hostname === "example.org")
      outside.push(r.url());
  });
  page.on("console", (m) => {
    if (/Content Security Policy/i.test(m.text())) violations.push(m.text());
  });
  // Bob's session, as the sign-in leaves it in a tab (RFC 0003 decision 17).
  // Once: on the next load the tab holds the rotated one (decision 26 would
  // rightly end a session that reused the first).
  await page.addInitScript((token) => {
    if (!sessionStorage.getItem("kura:refresh-token"))
      sessionStorage.setItem("kura:refresh-token", token);
  }, env("KURA_E2E_REFRESH"));

  // 1. The tree shows the shared folders only, and the note renders.
  await page.goto(`/app/vaults/${vault}/notes/${env("KURA_E2E_NOTE")}`);
  const tree = page.locator("[data-test=tree]");
  await expect(tree).toContainText("Sessions");
  await expect(tree).toContainText("Resumes");
  await expect(tree).not.toContainText("MJ");
  const frame = page.frameLocator("[data-test=note-frame]");
  await expect(frame.locator("h1")).toHaveText("Session 12");
  // [[Session 11]] and [[MJ/secret]]: plain text, no link, the same for both.
  await expect(frame.locator(".unshared")).toHaveCount(2);
  await expect(frame.locator(".unshared").first()).toHaveAttribute(
    "title",
    "not shared with you",
  );
  await expect(frame.locator("a")).toHaveCount(0);

  // 6. The note's payloads ran nothing and called nobody; the frame is an
  // opaque origin that cannot reach the application's storage.
  await page.waitForTimeout(1_000);
  expect(outside).toEqual([]);
  const storage = await page
    .frameLocator("[data-test=note-frame]")
    .locator("body")
    .evaluate(() => {
      try {
        return String(parent.localStorage.length);
      } catch (e) {
        return e instanceof Error ? e.name : String(e);
      }
    });
  expect(storage).toBe("SecurityError");
  // A script slipped past the sanitiser by hand is refused by the policy.
  await page.evaluate(() => {
    const f = document.querySelector<HTMLIFrameElement>(
      "[data-test=note-frame]",
    );
    if (f) f.srcdoc = "<p>injected</p><script>fetch('/x')</script>";
  });
  await page.waitForTimeout(1_000);
  expect(outside).toEqual([]);
  expect(violations.some((v) => /script-src/.test(v))).toBe(true);

  // 3 (page half). The search page names the limit of what bob sees.
  await page.goto(`/app/vaults/${vault}/search`);
  await page.locator("[data-test=query]").fill("xyzzy");
  await page.getByRole("button", { name: "Search" }).click();
  await expect(page.locator("[data-test=no-hit]")).toHaveText(
    'No note matches "xyzzy" in the folders shared with you.',
  );

  // 5 (page half). The hidden note's address says it does not exist.
  await page.goto(`/app/vaults/${vault}/notes/${env("KURA_E2E_HIDDEN")}`);
  await expect(page.locator("[data-test=not-found]")).toContainText(
    "does not exist, or is not shared with you",
  );
  await signOut(page);
});
