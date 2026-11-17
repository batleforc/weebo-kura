// The console's service worker (RFC 0003 §6.1): it makes the application
// installable and keeps its shell (the hashed assets and index.html) for a
// fast start. It never stores an API answer, config.json or anything else,
// and it never sees a credential: requests it does not serve from the cache
// go to the network untouched.

const CACHE = "kura-shell-v1";
const SCOPE = "/app/";

/** Whether a GET to this same-origin path may be served from, or kept in, the cache. */
function cacheable(url) {
  if (url.origin !== self.location.origin) return false;
  if (url.search !== "") return false;
  return url.pathname.startsWith(`${SCOPE}assets/`) || url.pathname === SCOPE;
}

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches
      .open(CACHE)
      .then((cache) => cache.add(SCOPE))
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(
          keys.filter((k) => k !== CACHE).map((k) => caches.delete(k)),
        ),
      )
      .then(() => self.clients.claim()),
  );
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  const url = new URL(request.url);
  // A client route of the application gets the shell; everything else that
  // is not the shell passes through, never cached.
  const navigation =
    request.mode === "navigate" && url.pathname.startsWith(SCOPE);
  if (request.method !== "GET" || (!navigation && !cacheable(url))) return;
  const key = navigation ? SCOPE : request;
  event.respondWith(
    caches.open(CACHE).then(async (cache) => {
      try {
        const fresh = await fetch(request);
        if (fresh.ok && fresh.type === "basic")
          await cache.put(key, fresh.clone());
        return fresh;
      } catch (error) {
        const kept = await cache.match(key);
        if (kept) return kept;
        throw error;
      }
    }),
  );
});

if (typeof module !== "undefined") module.exports = { cacheable };
