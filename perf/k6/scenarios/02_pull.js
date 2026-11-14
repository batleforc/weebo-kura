// A device's pull: every page of `changes` from a cursor near the head, the
// call each notification triggers (§6.3). Five devices, about 8 pulls/s,
// under the 600/min limit (§4.4).
// Run: task perf:run:pull
import http from "k6/http";
import { sleep } from "k6";
import { check, headers, pullAll, registerDevices, vaultUrl } from "../helpers.js";

const DEVICES = 5;

export const options = {
  scenarios: {
    pull: { executor: "constant-vus", vus: DEVICES, duration: __ENV.PERF_DURATION || "1m" },
  },
  thresholds: {
    http_req_failed: ["rate<0.01"],
    "http_req_duration{name:changes}": ["p(95)<100"],
  },
};

export function setup() {
  const devices = registerDevices(DEVICES);
  const head = http.get(vaultUrl("/changes?since=0"), { headers: headers(devices[0]) }).json("headSeq");
  return { devices, since: Math.max(0, head - 20) };
}

export default function (data) {
  const device = data.devices[(__VU - 1) % DEVICES];
  check(pullAll(device, data.since), { "pulled": (n) => n >= 0 });
  sleep(0.6);
}

