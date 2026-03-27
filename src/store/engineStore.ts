import { create } from "zustand";

export type EngineStatus = "OFFLINE" | "STARTING" | "ACTIVE";
const VOSK_MODEL_STORAGE_KEY = "hellcall.selectedVoskModelId";
const DEFAULT_VOSK_MODEL_ID = "vosk-model-small-cn-0.22";
const VISION_MODEL_STORAGE_KEY = "hellcall.selectedVisionModelId";
const DEFAULT_VISION_MODEL_ID = "helldivers2-yolo-v8n";

const getStoredModelId = (storageKey: string, fallback: string) => {
  if (typeof window === "undefined") {
    return fallback;
  }

  return window.localStorage.getItem(storageKey) || fallback;
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
  selectedVisionModelId: string;
  setSelectedVisionModelId: (modelId: string) => void;
  selectedVisionModelReady: boolean | null;
  setSelectedVisionModelReady: (ready: boolean | null) => void;
}

export const useEngineStore = create<EngineState>((set) => ({
  status: "OFFLINE",
  setStatus: (status) => set({ status }),
  selectedDevice: null,
  setSelectedDevice: (device) => set({ selectedDevice: device }),
  selectedVoskModelId: getStoredModelId(
    VOSK_MODEL_STORAGE_KEY,
    DEFAULT_VOSK_MODEL_ID,
  ),
  setSelectedVoskModelId: (modelId) => {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(VOSK_MODEL_STORAGE_KEY, modelId);
    }
    set({ selectedVoskModelId: modelId });
  },
  selectedVoskModelReady: null,
  setSelectedVoskModelReady: (ready) => set({ selectedVoskModelReady: ready }),
  selectedVisionModelId: getStoredModelId(
    VISION_MODEL_STORAGE_KEY,
    DEFAULT_VISION_MODEL_ID,
  ),
  setSelectedVisionModelId: (modelId) => {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(VISION_MODEL_STORAGE_KEY, modelId);
    }
    set({ selectedVisionModelId: modelId });
  },
  selectedVisionModelReady: null,
  setSelectedVisionModelReady: (ready) =>
    set({ selectedVisionModelReady: ready }),
}));
