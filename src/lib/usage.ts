import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { UsageSnapshot } from "./types";

export async function getUsage(): Promise<UsageSnapshot[]> {
  return invoke("get_usage");
}

/**
 * Subscribes to the backend's 60s poll loop instead of polling from the
 * frontend — the loop keeps running even while this window isn't mounted,
 * so callback fires with fresh data as soon as the listener attaches.
 */
export async function onUsageUpdated(
  callback: (snapshots: UsageSnapshot[]) => void,
): Promise<UnlistenFn> {
  return listen<UsageSnapshot[]>("usage://updated", (event) => {
    callback(event.payload);
  });
}
