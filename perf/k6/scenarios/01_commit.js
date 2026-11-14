// A one-note commit (RFC 0001 §10.3: under 200 ms at p95). Five devices,
// each committing about every 0.6 s, under the 120/min commit limit (§4.4):
// about 8 commits/s in all. Content is an existing blob, so the measure is
// the commit itself: lock, seq, revision, NOTIFY, idempotency record.
// Run: task perf:run:commit
import http from "k6/http";
import { sleep } from "k6";
import { BLOB, SIZE } from "../config.js";
import { check, headers, registerDevices, uuidv7, vaultUrl } from "../helpers.js";

const DEVICES = 5;

export const options = {
  scenarios: {
    commit: { executor: "constant-vus", vus: DEVICES, duration: __ENV.PERF_DURATION || "1m" },
  },
  thresholds: {
    http_req_failed: ["rate<0.01"],
    "http_req_duration{name:commit}": ["p(95)<200", "p(99)<500"],
  },
};

export function setup() {
  return { devices: registerDevices(DEVICES) };
}

export default function (data) {
  const device = data.devices[(__VU - 1) % DEVICES];
  const entry = uuidv7();
  const body = JSON.stringify({
    idempotencyKey: uuidv7(),
    operations: [{ op: "put", entry, path: `perf/${__VU}/${entry}.md`, blob: BLOB, size: SIZE }],
  });
  const r = http.post(vaultUrl("/commits"), body, { headers: headers(device), tags: { name: "commit" } });
  check(r, {
    "commit 200": (x) => x.status === 200,
    "accepted": (x) => x.status === 200 && x.json("results.0.outcome") === "accepted",
  });
  sleep(0.6);
}

