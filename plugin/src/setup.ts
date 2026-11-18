// The console's "Open in Obsidian" link (obsidian://kura-setup?…): the
// connection settings, read field by field. A link is untrusted input: it is
// shown to the person and applied only once they accept it.
import { secureUrl } from "./auth";

export interface Setup {
  server: string;
  issuer: string;
  clientId: string;
  /** The Kura vault's id and name; empty when the link names none. */
  vault: string;
  vaultName: string;
}

const CLIENT = /^[A-Za-z0-9._-]{1,64}$/;
const VAULT_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

/** The settings a link carries, or why it is refused. */
export function parseSetup(p: Record<string, string | undefined>): Setup | string {
  const server = (p.server ?? "").replace(/\/+$/, "");
  const issuer = p.issuer ?? "";
  const clientId = p.clientId ?? "kura-plugin";
  // Obsidian keeps `vault` for its own vault name: Kura's are prefixed.
  const vault = p.kuraVault ?? "";
  const vaultName = p.kuraVaultName ?? "";
  if (!secureUrl(server)) return "the server address is not https://";
  if (!secureUrl(issuer)) return "the identity provider address is not https://";
  if (!CLIENT.test(clientId)) return "the client id is not valid";
  if (vault !== "" && !VAULT_ID.test(vault)) return "the vault id is not valid";
  if (vaultName.length > 64) return "the vault name is too long";
  return { server, issuer, clientId, vault, vaultName: vault === "" ? "" : vaultName };
}
