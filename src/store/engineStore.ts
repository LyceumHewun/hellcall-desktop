import { create } from "zustand";

export type EngineStatus = "OFFLINE" | "STARTING" | "ACTIVE";
const VOSK_MODEL_STORAGE_KEY = "hellcall.selectedVoskModelId";
const DEFAULT_VOSK_MODEL_ID = "vosk-model-small-cn-0.22";

const getStoredVoskModelId = () => {
  if (typeof window === "undefined") {
    return DEFAULT_VOSK_MODEL_ID;
  }

  return (
    window.localStorage.getItem(VOSK_MODEL_STORAGE_KEY) || DEFAULT_VOSK_MODEL_ID
  );
};

interface EngineState {
  status: EngineStatus;
  setStatus: (status: EngineStatus) => void;
  selectedDevice: string | null;
  setSelectedDevice: (device: string | null) => void;
  selectedVoskModelId: string;
  setSelectedVoskModelId: (modelId: string) => void;
  selectedVoskModelReady: boolean | null;
  setSelectedVoskModelReady: (ready: boolean | null) => void;
}

export const useEngineStore = create<EngineState>((set) => ({
  status: "OFFLINE",
  setStatus: (status) => set({ status }),
  selectedDevice: null,
  setSelectedDevice: (device) => set({ selectedDevice: device }),
  selectedVoskModelId: getStoredVoskModelId(),
  setSelectedVoskModelId: (modelId) => {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(VOSK_MODEL_STORAGE_KEY, modelId);
    }
    set({ selectedVoskModelId: modelId });
  },
  selectedVoskModelReady: null,
  setSelectedVoskModelReady: (ready) => set({ selectedVoskModelReady: ready }),
}));
