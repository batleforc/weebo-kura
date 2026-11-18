/**
 * What the back office offers this caller (RFC 0003 §4.5 shell): the vaults
 * it manages, the tenants it administers, the platform when it is a
 * platform admin. Shown, never trusted: every action is a `/v1` call the
 * server authorises (§4.2).
 */

import type { Me, Vault } from "@/api/generated/types.gen";

export interface Targets {
  vaults: Vault[];
  tenants: string[];
  platform: boolean;
}

export function targets(me: Me | null, vaults: readonly Vault[]): Targets {
  const tenants = (me?.tenants ?? [])
    .filter((t) => t.role === "admin" || t.role === "owner")
    .map((t) => t.slug);
  return {
    vaults: vaults.filter(
      (v) => v.role === "manager" || tenants.includes(v.tenant),
    ),
    tenants,
    platform: me?.platformAdmin ?? false,
  };
}

export function anything(t: Targets): boolean {
  return t.vaults.length > 0 || t.tenants.length > 0 || t.platform;
}
