import { describe, expect, it } from "vitest";

import {
  base64url,
  beginAuthorization,
  challengeOf,
  inAppTarget,
  randomToken,
} from "./pkce";

describe("pkce", () => {
  it("computes the S256 challenge of RFC 7636 appendix B", async () => {
    expect(
      await challengeOf("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
    ).toBe("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
  });

  it("encodes base64url without padding", () => {
    expect(base64url(new Uint8Array([251, 255]))).toBe("-_8");
    expect(randomToken(32)).toMatch(/^[A-Za-z0-9_-]{43}$/);
  });

  it("starts with a fresh verifier and state", async () => {
    const a = await beginAuthorization("/vaults");
    const b = await beginAuthorization("/vaults");
    expect(a.pending.verifier).not.toBe(b.pending.verifier);
    expect(a.challenge).toBe(await challengeOf(a.pending.verifier));
  });

  // T9, `post_login_target_is_in_app`.
  it.each([
    ["/devices", "/devices"],
    ["/vaults?x=1", "/vaults?x=1"],
    ["https://evil.example/", "/"],
    ["//evil.example/", "/"],
    ["/\\evil.example", "/"],
    ["javascript:alert(1)", "/"],
    ["", "/"],
    [null, "/"],
  ])("keeps %s in the application as %s", (target, want) => {
    expect(inAppTarget(target)).toBe(want);
  });
});
