import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { StratagemCatalog } from "../types/stratagems";

interface StratagemsState {
  catalog: StratagemCatalog | null;
  isLoading: boolean;
  isRefreshing: boolean;
  hasLoaded: boolean;
  fetchCatalog: (force?: boolean) => Promise<void>;
  refreshCatalog: () => Promise<StratagemCatalog>;
}

export const useStratagemsStore = create<StratagemsState>((set, get) => ({
  catalog: null,
  isLoading: false,
  isRefreshing: false,
  hasLoaded: false,

  fetchCatalog: async (force = false) => {
    const { hasLoaded, isLoading } = get();
    if ((!force && hasLoaded) || isLoading) {
      return;
    }

    try {
      set({ isLoading: true });
      const catalog = await invoke<StratagemCatalog>("load_stratagems");
      set({
        catalog,
        isLoading: false,
        hasLoaded: true,
      });
    } catch (error) {
      set({ isLoading: false, hasLoaded: true });
      throw error;
    }
  },

  refreshCatalog: async () => {
    try {
      set({ isRefreshing: true });
      const catalog = await invoke<StratagemCatalog>("refresh_stratagems");
      set({
        catalog,
        isRefreshing: false,
        hasLoaded: true,
      });
      return catalog;
    } catch (error) {
      set({ isRefreshing: false });
      throw error;
    }
  },
}));
