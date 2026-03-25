import { create } from "zustand";

export type EngineStatus = "OFFLINE" | "STARTING" | "ACTIVE";

interface EngineState {
  status: EngineStatus;
  setStatus: (status: EngineStatus) => void;
  selectedDevice: string | null;
  setSelectedDevice: (device: string | null) => void;
}

export const useEngineStore = create<EngineState>((set) => ({
  status: "OFFLINE",
  setStatus: (status) => set({ status }),
  selectedDevice: null,
  setSelectedDevice: (device) => set({ selectedDevice: device }),
}));
