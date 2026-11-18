// RFC 0006 §10.3: the vault's device list, each row counting the notes its
// device is behind. Five devices caught up from 0 on `notes`, then read the
// list as the console does (a page load), about 8 reads/s, under the 60/min
// limit per device.
// Run: task perf:run:sync-state
import http from "k6/http";
import { sleep } from "k6";
import { check, headers, pullAll, registerDevices, vaultUrl } from "../helpers.js";

const DEVICES = 5;

export const options = {
  scenarios: {
    list: { executor: "constant-vus", vus: DEVICES, duration: __ENV.PERF_DURATION || "1m" },
  },
  thresholds: {
    http_req_failed: ["rate<0.01"],
    "http_req_duration{name:sync}": ["p(95)<100"],
  },
};

export function setup() {
  const devices = registerDevices(DEVICES);
  // Each device finishes one pass, so each has a row to count.
  for (const d of devices) pullAll(d, 0);
  return { devices };
}

export default function (data) {
  const device = data.devices[(__VU - 1) % DEVICES];
  const r = http.get(vaultUrl("/sync"), { headers: headers(device), tags: { name: "sync" } });
  check(r, { "200 with rows": (x) => x.status === 200 && x.json().length >= 1 });
  // 60 a minute per device (§4.4): one read a second at most.
  sleep(1.1);
}
