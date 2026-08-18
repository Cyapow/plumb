// Transport seam.
//
// Every backend call in the app goes through `invoke(cmd, args)` and every push
// through `listen(event, cb)`. Routing them here (instead of importing the Tauri
// APIs directly) gives us a single place to swap transports later — a Tauri IPC
// implementation today, a `plumb serve` HTTP+WebSocket implementation when the
// UI runs embedded in an editor. Call sites never change.
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type EventCallback, type UnlistenFn } from "@tauri-apps/api/event";

export type { UnlistenFn, EventCallback };

/** Call a backend command and resolve with its result. */
export function invoke<T = unknown>(command: string, args?: Record<string, unknown>): Promise<T> {
  return tauriInvoke<T>(command, args);
}

/** Subscribe to a backend event; resolves to an unlisten function. */
export function listen<T = unknown>(event: string, handler: EventCallback<T>): Promise<UnlistenFn> {
  return tauriListen<T>(event, handler);
}
