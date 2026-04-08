import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { AiLiveToolActivity, AiSessionRecord } from "../types/ai";

const DEFAULT_AI_SESSION_ID = "default-session";
export type AiUiPhase =
  | "idle"
  | "listening"
  | "transcribing"
  | "thinking"
  | "tool_running"
  | "speaking"
  | "error";

interface AiState {
  currentSessionId: string;
  currentSession: AiSessionRecord | null;
  liveToolActivities: AiLiveToolActivity[];
  isRecording: boolean;
  isStreaming: boolean;
  isSpeaking: boolean;
  phase: AiUiPhase;
  streamingText: string;
  lastTranscript: string | null;
  isLoadingSession: boolean;
  error: string | null;
  setError: (error: string | null) => void;
  clearError: () => void;
  setRecording: (recording: boolean) => void;
  setStreaming: (streaming: boolean) => void;
  setSpeaking: (speaking: boolean) => void;
  setPhase: (phase: AiUiPhase) => void;
  appendStreamingText: (delta: string) => void;
  resetStreamingText: () => void;
  pushLiveToolActivity: (activity: AiLiveToolActivity) => void;
  resetLiveToolActivities: () => void;
  setLastTranscript: (transcript: string | null) => void;
  fetchSession: () => Promise<void>;
  resetSession: () => Promise<void>;
}

export const useAiStore = create<AiState>((set) => ({
  currentSessionId: DEFAULT_AI_SESSION_ID,
  currentSession: null,
  liveToolActivities: [],
  isRecording: false,
  isStreaming: false,
  isSpeaking: false,
  phase: "idle",
  streamingText: "",
  lastTranscript: null,
  isLoadingSession: false,
  error: null,
  setError: (error) => set({ error }),
  clearError: () => set({ error: null }),
  setRecording: (recording) => set({ isRecording: recording }),
  setStreaming: (streaming) => set({ isStreaming: streaming }),
  setSpeaking: (speaking) => set({ isSpeaking: speaking }),
  setPhase: (phase) => set({ phase }),
  appendStreamingText: (delta) =>
    set((state) => ({ streamingText: `${state.streamingText}${delta}` })),
  resetStreamingText: () => set({ streamingText: "" }),
  pushLiveToolActivity: (activity) =>
    set((state) => ({
      liveToolActivities: [...state.liveToolActivities, activity],
    })),
  resetLiveToolActivities: () => set({ liveToolActivities: [] }),
  setLastTranscript: (transcript) => set({ lastTranscript: transcript }),

  fetchSession: async () => {
    try {
      set({ isLoadingSession: true, error: null });
      const session = await invoke<AiSessionRecord>("get_ai_session", {
        sessionId: DEFAULT_AI_SESSION_ID,
      });
      set({
        currentSession: session,
        isLoadingSession: false,
      });
    } catch (error) {
      set({
        isLoadingSession: false,
        error: error instanceof Error ? error.message : String(error ?? ""),
      });
    }
  },

  resetSession: async () => {
    try {
      set({ error: null });
      await invoke("delete_ai_session", { sessionId: DEFAULT_AI_SESSION_ID });
      const session = await invoke<AiSessionRecord>("get_ai_session", {
        sessionId: DEFAULT_AI_SESSION_ID,
      });
      set({
        currentSession: session,
        streamingText: "",
        liveToolActivities: [],
      });
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : String(error ?? ""),
      });
    }
  },
}));
