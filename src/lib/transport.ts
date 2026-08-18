// Transport seam.
//
// Every backend call goes through `invoke(cmd, args)` and every push through
// `listen(event, cb)`. Two implementations live behind these: Tauri IPC (the
// desktop app) and an HTTP client for `plumb serve` (the app embedded in an
// editor webview or a plain browser tab). The implementation is chosen once at
// load from whether a serve endpoint was handed to the page; call sites never
// change.
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type EventCallback, type UnlistenFn } from "@tauri-apps/api/event";

export type { UnlistenFn, EventCallback };

interface ServeConfig {
  origin: string;
  token: string;
  repo?: string;
}

/**
 * Discover a `plumb serve` endpoint: injected as `window.__PLUMB__`, or passed
 * as `?serve=PORT.TOKEN` (with optional `&repo=/path`) and remembered for the
 * session so navigation keeps it.
 */
function readServe(): ServeConfig | null {
  // A ?repo= in the URL always wins, so a shared agent can open a different
  // repo per editor panel / tab.
  const urlRepo = new URLSearchParams(location.search).get("repo") ?? undefined;
  const injected = (globalThis as unknown as { __PLUMB__?: { port: number; token: string; repo?: string } }).__PLUMB__;
  if (injected?.port && injected.token) {
    return { origin: `http://127.0.0.1:${injected.port}`, token: injected.token, repo: urlRepo ?? injected.repo };
  }
  const params = new URLSearchParams(location.search);
  const q = params.get("serve");
  const stored = sessionStorage.getItem("plumb.serve");
  const raw = q ?? stored;
  if (!raw) return null;
  if (q) sessionStorage.setItem("plumb.serve", q);
  const [port, token] = raw.split(".");
  if (!port || !token) return null;
  const repo = params.get("repo") ?? sessionStorage.getItem("plumb.repo") ?? undefined;
  if (params.get("repo")) sessionStorage.setItem("plumb.repo", params.get("repo")!);
  return { origin: `http://127.0.0.1:${port}`, token, repo };
}

const serve = readServe();

/** True when the app is driving a `plumb serve` backend rather than Tauri IPC. */
export const isServed = !!serve;
/** A repo path the serve session was launched with, if any. */
export const servedRepo = serve?.repo;

async function httpInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const res = await fetch(`${serve!.origin}/rpc`, {
    method: "POST",
    headers: { "content-type": "application/json", "x-plumb-token": serve!.token },
    body: JSON.stringify({ command, args: args ?? {} }),
  });
  const data = (await res.json()) as { ok?: T; error?: string };
  if (data.error) throw new Error(data.error);
  return data.ok as T;
}

/** Call a backend command and resolve with its result. */
export function invoke<T = unknown>(command: string, args?: Record<string, unknown>): Promise<T> {
  return serve ? httpInvoke<T>(command, args) : tauriInvoke<T>(command, args);
}

// Served mode receives events by long-polling GET /events: the server blocks
// until events queue (or ~25s), returns them, and we immediately re-poll.
// Events queue per client id between polls, so none are dropped.
const eventListeners = new Map<string, Set<(e: { payload: unknown }) => void>>();
const clientId = Math.random().toString(36).slice(2) + Date.now().toString(36);
let pollStarted = false;

async function pollLoop() {
  const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
  while (serve) {
    try {
      const res = await fetch(`${serve.origin}/events?token=${encodeURIComponent(serve.token)}&id=${clientId}`);
      if (!res.ok) {
        await sleep(1000);
        continue;
      }
      const data = (await res.json()) as { events?: { event: string; payload: unknown }[] };
      for (const ev of data.events ?? []) {
        eventListeners.get(ev.event)?.forEach((cb) => cb({ payload: ev.payload }));
      }
    } catch {
      await sleep(1000);
    }
  }
}

function servedListen(event: string, handler: (e: { payload: unknown }) => void): UnlistenFn {
  if (!pollStarted) {
    pollStarted = true;
    void pollLoop();
  }
  let set = eventListeners.get(event);
  if (!set) {
    set = new Set();
    eventListeners.set(event, set);
  }
  set.add(handler);
  return () => {
    eventListeners.get(event)?.delete(handler);
  };
}

/** Subscribe to a backend event; resolves to an unlisten function. */
export function listen<T = unknown>(event: string, handler: EventCallback<T>): Promise<UnlistenFn> {
  if (serve) return Promise.resolve(servedListen(event, handler as (e: { payload: unknown }) => void));
  return tauriListen<T>(event, handler);
}
