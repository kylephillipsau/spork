/*
 * The application, kept on the device (D255): it opens from here, so a
 * handheld on poor wifi starts at once and a restart of the host is not a
 * blank screen.
 *
 * - The screens' document is asked of the network first, and the copy kept
 *   answers when the network does not. It names today's assets. The
 *   documents the server draws (/print, /app, /pair) are left to it.
 * - Assets are named by the hash of their bytes, so a copy kept is never
 *   stale: from the device, then the network.
 * - /api is the network's alone. Writes wait in the outbox (D207), not here.
 *
 * Only on HTTPS, or at localhost: a browser offers a service worker nowhere
 * else.
 */
const SHELL = "spork-shell";
const ASSETS = "spork-assets";
/** Assets kept: the newest builds' worth, a deploy at a time. */
const KEEP = 80;

self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (event) => event.waitUntil(self.clients.claim()));

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") return;
  const url = new URL(request.url);
  if (url.origin !== self.location.origin || url.pathname.startsWith("/api/")) return;

  if (url.pathname.startsWith("/assets/")) {
    event.respondWith(fromCache(ASSETS, request));
    return;
  }
  if (request.mode === "navigate" && !/^\/(print|app|pair)(\/|$)/.test(url.pathname)) {
    event.respondWith(fromNetwork(request));
  }
});

async function fromCache(name, request) {
  const cache = await caches.open(name);
  const kept = await cache.match(request);
  if (kept) return kept;
  const response = await fetch(request);
  if (response.ok) {
    await cache.put(request, response.clone());
    const keys = await cache.keys();
    for (const old of keys.slice(0, Math.max(0, keys.length - KEEP))) await cache.delete(old);
  }
  return response;
}

async function fromNetwork(request) {
  const cache = await caches.open(SHELL);
  try {
    const response = await fetch(request);
    if (response.ok) await cache.put("/", response.clone());
    return response;
  } catch (error) {
    const kept = await cache.match("/");
    if (kept) return kept;
    throw error;
  }
}
