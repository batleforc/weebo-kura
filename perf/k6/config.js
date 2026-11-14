// The server under test and its fixtures, from perf/run.sh (task perf:run:*).
export const BASE_URL = __ENV.PERF_URL || "http://127.0.0.1:58080";
// An access token of a member with the editor role on PERF_VAULT. Every
// device of a run shares its session, so perf/run.sh revokes them after the
// run (revoking one through the API ends the session, decision 35).
export const TOKEN = __ENV.PERF_TOKEN || "";
export const VAULT = __ENV.PERF_VAULT || "";
// A blob already stored in the vault (hex) and its size: commits reference it,
// so a commit costs what the server does, not uploads k6 cannot hash.
export const BLOB = __ENV.PERF_BLOB || "";
export const SIZE = Number(__ENV.PERF_SIZE || "0");
