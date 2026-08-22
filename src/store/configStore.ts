import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { AppConfig } from "../types/config";
import { sanitizeConfigForEngine } from "./engineConfig";

interface ConfigState {
  config: AppConfig | null;
  isLoading: boolean;
  fetchConfig: () => Promise<void>;
  updateConfig: (
    updater: (draft: AppConfig) => void | Partial<AppConfig>,
  ) => void;
  updateConfigImmediately: (
    updater: (draft: AppConfig) => void | Partial<AppConfig>,
  ) => Promise<void>;
}

let saveTimeout: ReturnType<typeof setTimeout> | null = null;

function createUpdatedConfig(
  config: AppConfig,
  updater: (draft: AppConfig) => void | Partial<AppConfig>,
) {
  const newConfig = JSON.parse(JSON.stringify(config)) as AppConfig;
  const result = updater(newConfig);
  if (result) Object.assign(newConfig, result);
  return newConfig;
}

async function saveConfig(config: AppConfig) {
  const sanitizedConfig = sanitizeConfigForEngine(config);
  await invoke("save_config", { newConfig: sanitizedConfig });
}

export const useConfigStore = create<ConfigState>((set, get) => ({
  config: null,
  isLoading: true,

  fetchConfig: async () => {
    try {
      set({ isLoading: true });
      const config = await invoke<AppConfig>("load_config");
      // Add a stable frontend ID to each command to prevent focus loss during DnD
      config.commands.forEach((cmd) => {
        cmd._frontendId = crypto.randomUUID();
      });
      set({ config, isLoading: false });
    } catch (error) {
      console.error("Failed to load config:", error);
      set({ isLoading: false });
    }
  },

  updateConfig: (updater) => {
    set((state) => {
      if (!state.config) return state;

      const newConfig = createUpdatedConfig(state.config, updater);

      // Debounce saving to backend
      if (saveTimeout) {
        clearTimeout(saveTimeout);
      }

      saveTimeout = setTimeout(async () => {
        try {
          await saveConfig(newConfig);
          console.log("Config saved successfully");
        } catch (error) {
          console.error("Failed to save config:", error);
          get().fetchConfig(); // Recover state from backend
        }
      }, 500);
      return { config: newConfig };
    });
  },

  updateConfigImmediately: async (updater) => {
    const config = get().config;
    if (!config) return;

    if (saveTimeout) {
      clearTimeout(saveTimeout);
      saveTimeout = null;
    }

    const newConfig = createUpdatedConfig(config, updater);
    set({ config: newConfig });

    try {
      await saveConfig(newConfig);
    } catch (error) {
      console.error("Failed to save config:", error);
      await get().fetchConfig();
      throw error;
    }
  },
}));
