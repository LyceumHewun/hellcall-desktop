import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { AiSessionRecord, AiSessionSummary } from "../types/ai";

interface AiState {
  sessions: AiSessionSummary[];
  currentSessionId: string | null;
  currentSession: AiSessionRecord | null;
  isRecording: boolean;
  isStreaming: boolean;
  streamingText: string;
  lastTranscript: string | null;
  isLoadingSessions: boolean;
  isLoadingSession: boolean;
  error: string | null;
  setError: (error: string | null) => void;
  clearError: () => void;
  setRecording: (recording: boolean) => void;
  setStreaming: (streaming: boolean) => void;
  appendStreamingText: (delta: string) => void;
  resetStreamingText: () => void;
  setLastTranscript: (transcript: string | null) => void;
  fetchSessions: () => Promise<void>;
  selectSession: (sessionId: string) => Promise<void>;
  createSession: (title?: string) => Promise<void>;
  deleteSession: (sessionId: string) => Promise<void>;
}

export const useAiStore = create<AiState>((set, get) => ({
  sessions: [],
  currentSessionId: null,
  currentSession: null,
  isRecording: false,
  isStreaming: false,
  streamingText: "",
  lastTranscript: null,
  isLoadingSessions: false,
  isLoadingSession: false,
  error: null,
  setError: (error) => set({ error }),
  clearError: () => set({ error: null }),
  setRecording: (recording) => set({ isRecording: recording }),
  setStreaming: (streaming) => set({ isStreaming: streaming }),
  appendStreamingText: (delta) =>
    set((state) => ({ streamingText: `${state.streamingText}${delta}` })),
  resetStreamingText: () => set({ streamingText: "" }),
  setLastTranscript: (transcript) => set({ lastTranscript: transcript }),

  fetchSessions: async () => {
    try {
      set({ isLoadingSessions: true, error: null });
      const sessions = await invoke<AiSessionSummary[]>("list_ai_sessions");
      const { currentSessionId } = get();
      const nextCurrentId =
        currentSessionId &&
        sessions.some((session) => session.id === currentSessionId)
          ? currentSessionId
          : sessions[0]?.id ?? null;

      set({
        sessions,
        currentSessionId: nextCurrentId,
        streamingText: "",
        isLoadingSessions: false,
      });

      if (nextCurrentId) {
        await get().selectSession(nextCurrentId);
      } else {
        set({ currentSession: null });
      }
    } catch (error) {
      set({
        isLoadingSessions: false,
        error: error instanceof Error ? error.message : String(error ?? ""),
      });
    }
  },

  selectSession: async (sessionId) => {
    try {
      set({ isLoadingSession: true, error: null, currentSessionId: sessionId });
      const session = await invoke<AiSessionRecord>("get_ai_session", {
        sessionId,
      });
      set({ currentSession: session, isLoadingSession: false, streamingText: "" });
    } catch (error) {
      set({
        isLoadingSession: false,
        error: error instanceof Error ? error.message : String(error ?? ""),
      });
    }
  },

  createSession: async (title) => {
    try {
      set({ error: null });
      const session = await invoke<AiSessionSummary>("create_ai_session", {
        title: title ?? null,
      });
      set((state) => ({
        sessions: [session, ...state.sessions],
        currentSessionId: session.id,
        streamingText: "",
      }));
      await get().selectSession(session.id);
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : String(error ?? ""),
      });
    }
  },

  deleteSession: async (sessionId) => {
    try {
      set({ error: null });
      await invoke("delete_ai_session", { sessionId });

      const { currentSessionId, sessions } = get();
      const remaining = sessions.filter((session) => session.id !== sessionId);
      const nextCurrentId =
        currentSessionId === sessionId ? remaining[0]?.id ?? null : currentSessionId;

      set({
        sessions: remaining,
        currentSessionId: nextCurrentId,
        currentSession: null,
        streamingText: "",
      });

      if (nextCurrentId) {
        await get().selectSession(nextCurrentId);
      }
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : String(error ?? ""),
      });
    }
  },
}));
