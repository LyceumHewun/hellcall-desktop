import { invoke } from "@tauri-apps/api/core";
import { AppConfig } from "../types/config";

// Function to fetch config on startup
export async function fetchConfig(): Promise<AppConfig> {
  return await invoke("load_config");
}

// Function to save config when user clicks save
export async function saveConfig(config: AppConfig): Promise<void> {
  await invoke("save_config", { newConfig: config });
}
