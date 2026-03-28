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
}

let saveTimeout: ReturnType<typeof setTimeout> | null = null;

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

      const newConfig = JSON.parse(JSON.stringify(state.config));

      const result = updater(newConfig);
      if (result) {
        Object.assign(newConfig, result);
      }

      // Debounce saving to backend
      if (saveTimeout) {
        clearTimeout(saveTimeout);
      }

      saveTimeout = setTimeout(async () => {
        try {
          const sanitizedConfig = sanitizeConfigForEngine(newConfig);
          await invoke("save_config", { newConfig: sanitizedConfig });
          console.log("Config saved successfully");
        } catch (error) {
          console.error("Failed to save config:", error);
          get().fetchConfig(); // Recover state from backend
        }
      }, 500);
      return { config: newConfig };
    });
  },
}));
