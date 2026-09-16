import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AgentId, Settings } from "./types";

export interface PlanTierOption {
  key: string;
  label: string;
}

export async function listPlanTiers(agent: AgentId): Promise<PlanTierOption[]> {
  return invoke("list_plan_tiers", { agent });
}

export async function getSettings(): Promise<Settings> {
  return invoke("get_settings");
}

export async function updateSettings(settings: Settings): Promise<void> {
  return invoke("update_settings", { newSettings: settings });
}

export async function openSettingsWindow(): Promise<void> {
  return invoke("open_settings_window");
}

export async function onSettingsChanged(
  callback: (settings: Settings) => void,
): Promise<UnlistenFn> {
  return listen<Settings>("settings-changed", (event) => {
    callback(event.payload);
  });
}
