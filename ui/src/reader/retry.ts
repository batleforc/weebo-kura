/**
 * The reader's "server unavailable" state (RFC 0005 §4.5): a 503 retries by
 * itself every 30 s, the Retry button stays for the impatient.
 */

import axios from "axios";
import { onBeforeUnmount } from "vue";

import { messageOf } from "@/api/problem";
import { i18n } from "@/i18n";
import { ContentError } from "./content";

export const RETRY_MS = 30_000;

function unavailable(error: unknown): boolean {
  if (error instanceof ContentError) return error.status === 503;
  return axios.isAxiosError(error) && error.response?.status === 503;
}

/** What to show for a failed load; a 503 also schedules `retry` in 30 s. */
export function useAutoRetry(retry: () => unknown): (error: unknown) => string {
  let timer: ReturnType<typeof setTimeout> | undefined;
  onBeforeUnmount(() => clearTimeout(timer));
  return (error) => {
    clearTimeout(timer);
    if (!unavailable(error)) return messageOf(error);
    timer = setTimeout(() => void retry(), RETRY_MS);
    return i18n.global.t("reader.unavailable");
  };
}
