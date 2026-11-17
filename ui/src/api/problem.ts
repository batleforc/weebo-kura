/** Refusals as readable messages (RFC 9457). Branch on `type`, never on `title`. */

import axios from "axios";

import { i18n } from "@/i18n";

/** `insufficient_role` from `/v1/problems/insufficient_role`; `undefined` for anything else. */
export function problemType(error: unknown): string | undefined {
  if (!axios.isAxiosError(error)) return undefined;
  const type: unknown = (error.response?.data as { type?: unknown } | undefined)
    ?.type;
  return typeof type === "string"
    ? type.replace(/^\/v1\/problems\//, "")
    : undefined;
}

/** The problem's `detail` (English, from the server); `undefined` without one. */
export function detailOf(error: unknown): string | undefined {
  if (!axios.isAxiosError(error)) return undefined;
  const detail: unknown = (
    error.response?.data as { detail?: unknown } | undefined
  )?.detail;
  return typeof detail === "string" && detail !== "" ? detail : undefined;
}

const KNOWN = new Set([
  "insufficient_role",
  "not_found",
  "quota_exceeded",
  "device_revoked",
  "rate_limited",
  "unavailable",
]);

/** What to show for a failed call; never throws. */
export function messageOf(error: unknown): string {
  const t = i18n.global.t;
  if (axios.isAxiosError(error) && !error.response) return t("problem.network");
  const type = problemType(error);
  if (type && KNOWN.has(type)) return t(`problem.${type}`);
  if (axios.isAxiosError(error)) {
    return (
      detailOf(error) ??
      t("problem.status", { status: error.response?.status ?? 0 })
    );
  }
  return error instanceof Error ? error.message : t("problem.unexpected");
}
