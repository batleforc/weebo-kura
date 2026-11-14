// Five devices answered 410 at once (§10.3), each then listing every entry
// from 0, as a full resync does. Measures the listing of the whole vault
// (1,000 entries a page) under concurrent load.
// Run: task perf:run:resync
import http from "k6/http";
import { Trend } from "k6/metrics";
import { check, headers, pullAll, registerDevices, vaultUrl } from "../helpers.js";

const DEVICES = 5;
const resync = new Trend("resync_duration", true);

export const options = {
  scenarios: {
    resync: { executor: "per-vu-iterations", vus: DEVICES, iterations: 3, maxDuration: "5m" },
  },
  thresholds: {
    // The 410s are expected; every other answer must succeed.
    "http_req_failed{name:changes}": ["rate<0.01"],
    resync_duration: ["p(95)<5000"],
  },
};

export function setup() {
  return { devices: registerDevices(DEVICES) };
}

export default function (data) {
  const device = data.devices[(__VU - 1) % DEVICES];
  const start = Date.now();
  // A cursor from an epoch the server no longer has.
  const gone = http.get(vaultUrl("/changes?since=1&epoch=00000000-0000-4000-8000-000000000000"), {
    headers: headers(device),
    tags: { name: "stale" },
    responseCallback: http.expectedStatuses(410),
  });
  check(gone, { "410 history_rewound": (r) => r.status === 410 && r.json("type") === "/v1/problems/history_rewound" });
  const n = pullAll(device, 0);
  resync.add(Date.now() - start);
  check(n, { "listed every entry": (x) => x > 0 });
}

