import { invoke } from "@tauri-apps/api/core";

export async function resizeMainWindow(height: number): Promise<void> {
  return invoke("resize_main_window", { height });
}
