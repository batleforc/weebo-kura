import { check, fail } from "k6";
import http from "k6/http";
import { BASE_URL, TOKEN, VAULT } from "./config.js";

const CLIENT = { "X-Kura-Client": "kura-perf/0.1.0" };

export function headers(device) {
  const h = { Authorization: `Bearer ${TOKEN}`, "Content-Type": "application/json", ...CLIENT };
  if (device) h["X-Kura-Device"] = device;
  return h;
}

/** One device per virtual user: the §4.4 limits count per device. */
export function registerDevices(n) {
  if (!TOKEN || !VAULT) fail("run through perf/run.sh: PERF_TOKEN and PERF_VAULT are unset");
  const out = [];
  for (let i = 0; i < n; i++) {
    const r = http.post(`${BASE_URL}/v1/devices`, JSON.stringify({ name: `perf-${__ENV.PERF_RUN || Date.now()}-${i}`, profile: "desktop" }), {
      headers: headers(),
      tags: { name: "register" },
    });
    if (r.status !== 201) fail(`device registration: ${r.status} ${r.body}`);
    out.push(r.json("id"));
  }
  return out;
}

/** A UUIDv7, as clients generate entry ids. */
export function uuidv7() {
  const b = new Uint8Array(16);
  crypto.getRandomValues(b);
  const ms = Date.now();
  for (let i = 0; i < 6; i++) b[i] = Math.floor(ms / 2 ** (8 * (5 - i))) % 256;
  b[6] = (b[6] & 0x0f) | 0x70;
  b[8] = (b[8] & 0x3f) | 0x80;
  const h = Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
}

export function vaultUrl(path) {
  return `${BASE_URL}/v1/vaults/${VAULT}${path}`;
}

/** Every page of `changes` from `since`; returns the number of items. */
export function pullAll(device, since, epoch) {
  let items = 0;
  let pos = since;
  for (;;) {
    // `start`: the pass's first cursor, so its later pages cross the horizon (§6.3).
    const q = `since=${pos}&start=${since}` + (epoch ? `&epoch=${epoch}` : "");
    const r = http.get(vaultUrl(`/changes?${q}`), { headers: headers(device), tags: { name: "changes" } });
    if (!check(r, { "changes 200": (x) => x.status === 200 && x.json("items") !== undefined })) return -1;
    items += r.json("items").length;
    const next = r.json("next");
    if (!next) return items;
    pos = Number(next);
  }
}

export { check };
