import { describe, expect, it } from "vitest";
import { parseSetup } from "../src/setup";

const id = "fd518771-93b3-46e8-ab0f-31bae844193e";

describe("the setup link", () => {
  it("reads the console's fields", () => {
    expect(
      parseSetup({ server: "https://kura.example.org/", issuer: "https://auth.example.org/application/o/kura-plugin/", clientId: "kura-plugin", kuraVault: id, kuraVaultName: "notes" }),
    ).toEqual({ server: "https://kura.example.org", issuer: "https://auth.example.org/application/o/kura-plugin/", clientId: "kura-plugin", vault: id, vaultName: "notes" });
  });

  it("allows loopback http, defaults the client, and may name no vault", () => {
    expect(parseSetup({ server: "http://127.0.0.1:58080", issuer: "https://localhost:59443/" })).toEqual({
      server: "http://127.0.0.1:58080",
      issuer: "https://localhost:59443/",
      clientId: "kura-plugin",
      vault: "",
      vaultName: "",
    });
  });

  it("refuses what it cannot trust", () => {
    expect(parseSetup({ server: "http://kura.example.org", issuer: "https://a.example/" })).toMatch(/server/);
    expect(parseSetup({ server: "https://k.example", issuer: "javascript:alert(1)" })).toMatch(/identity provider/);
    expect(parseSetup({ server: "https://k.example", issuer: "https://a.example/", clientId: "a b" })).toMatch(/client/);
    expect(parseSetup({ server: "https://k.example", issuer: "https://a.example/", kuraVault: "../x" })).toMatch(/vault id/);
    expect(parseSetup({})).toMatch(/server/);
  });
});
