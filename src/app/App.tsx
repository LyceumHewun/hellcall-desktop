import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { CustomTitlebar } from "./components/CustomTitlebar";
import { Sidebar } from "./components/Sidebar";
import { UpdaterDialog } from "./components/UpdaterDialog";
import { Loader2 } from "lucide-react";
import { GlobalSettingsView } from "./views/GlobalSettingsView";
import { KeyBindingsView } from "./views/KeyBindingsView";
import { MacrosView } from "./views/MacrosView";
import { LogView } from "./views/LogView";
import { StratagemsView } from "./views/StratagemsView";
import { AIView } from "./views/AIView";
import { Toaster, toast } from "sonner";
import { useConfigStore } from "../store/configStore";
import { useAiStore } from "../store/aiStore";
import { useEngineStore } from "../store/engineStore";

const toasterOptions = {
  style: {
    background: "var(--popover)",
    color: "var(--popover-foreground)",
  },
  classNames: {
    toast:
      "rounded-lg border border-border shadow-lg backdrop-blur-sm bg-popover text-popover-foreground",
    title: "text-foreground",
    description: "text-muted-foreground",
    actionButton: "!bg-primary !text-primary-foreground",
    cancelButton: "!bg-secondary !text-secondary-foreground",
    success: "!border-emerald-500/30",
    error: "!border-destructive/50",
    warning: "!border-amber-500/40",
    info: "!border-primary/40",
  },
} as const;

export default function App() {
  const { config, isLoading, fetchConfig } = useConfigStore();
  const {
    currentSessionId,
    appendStreamingText,
    clearError,
    fetchSession,
    pushLiveToolActivity,
    resetLiveToolActivities,
    resetStreamingText,
    setError,
    setLastTranscript,
    setPhase,
    setRecording,
    setSpeaking,
    setStreaming,
  } = useAiStore();
  const selectedDevice = useEngineStore((state) => state.selectedDevice);
  const [activeNav, setActiveNav] = useState("macros");

  useEffect(() => {
    fetchConfig();
  }, [fetchConfig]);

  useEffect(() => {
    if (!isLoading && config) {
      getCurrentWindow().show();
    }
  }, [isLoading, config]);

  useEffect(() => {
    if (!config) {
      return;
    }

    invoke("sync_ai_runtime_config", {
      config,
      deviceName: selectedDevice,
      sessionId: currentSessionId,
    }).catch((error) => {
      console.error("Failed to sync AI runtime config:", error);
    });
  }, [config, currentSessionId, selectedDevice]);

  useEffect(() => {
    let mounted = true;
    let unlistenRecording: UnlistenFn | null = null;
    let unlistenTranscript: UnlistenFn | null = null;
    let unlistenError: UnlistenFn | null = null;
    let unlistenChatState: UnlistenFn | null = null;
    let unlistenChatDelta: UnlistenFn | null = null;
    let unlistenChatFinished: UnlistenFn | null = null;
    let unlistenTtsState: UnlistenFn | null = null;
    let unlistenToolEvent: UnlistenFn | null = null;

    const recordingPromise = listen<{ recording: boolean }>(
      "ai-recording-state",
      (event) => {
        if (!mounted) {
          return;
        }
        setRecording(event.payload.recording);
        setPhase(event.payload.recording ? "listening" : "transcribing");
      },
    ).then((fn) => {
      unlistenRecording = fn;
      return fn;
    });

    const transcriptPromise = listen<{
      session_id: string;
      transcript: string;
    }>("ai-transcription-ready", async (event) => {
      if (!mounted) {
        return;
      }

      setLastTranscript(event.payload.transcript);
      clearError();
      resetStreamingText();
      resetLiveToolActivities();
      setPhase("thinking");
      await fetchSession();
    }).then((fn) => {
      unlistenTranscript = fn;
      return fn;
    });

    const errorPromise = listen<{ message: string }>("ai-recording-error", (event) => {
      if (!mounted) {
        return;
      }
      console.error("AI error:", event.payload.message);
      setRecording(false);
      setSpeaking(false);
      setStreaming(false);
      setPhase("error");
      setError(event.payload.message);
      toast.error(event.payload.message);
    }).then((fn) => {
      unlistenError = fn;
      return fn;
    });

    const chatStatePromise = listen<{ streaming: boolean }>("ai-chat-state", (event) => {
      if (!mounted) {
        return;
      }
      setStreaming(event.payload.streaming);
      setSpeaking(false);
      if (event.payload.streaming) {
        setPhase("thinking");
      } else {
        setPhase("idle");
        fetchSession().catch(console.error);
      }
    }).then((fn) => {
      unlistenChatState = fn;
      return fn;
    });

    const chatDeltaPromise = listen<{ session_id: string; delta: string }>(
      "ai-chat-delta",
      (event) => {
        if (!mounted || event.payload.session_id !== currentSessionId) {
          return;
        }
        appendStreamingText(event.payload.delta);
        setPhase("thinking");
      },
    ).then((fn) => {
      unlistenChatDelta = fn;
      return fn;
    });

    const chatFinishedPromise = listen<{ session_id: string; message: string }>(
      "ai-chat-finished",
      async (event) => {
        if (!mounted || event.payload.session_id !== currentSessionId) {
          return;
        }
        await fetchSession();
      },
    ).then((fn) => {
      unlistenChatFinished = fn;
      return fn;
    });

    const ttsStatePromise = listen<{ speaking: boolean }>("ai-tts-state", (event) => {
      if (!mounted) {
        return;
      }
      setSpeaking(event.payload.speaking);
      setPhase(event.payload.speaking ? "speaking" : "idle");
    }).then((fn) => {
      unlistenTtsState = fn;
      return fn;
    });

    const toolEventPromise = listen<{
      id: string;
      session_id: string;
      phase: "call" | "result" | "error";
      name: string;
      summary: string;
    }>("ai-tool-event", (event) => {
      if (!mounted || event.payload.session_id !== currentSessionId) {
        return;
      }
      pushLiveToolActivity(event.payload);
      setPhase(event.payload.phase === "call" ? "tool_running" : "thinking");
    }).then((fn) => {
      unlistenToolEvent = fn;
      return fn;
    });

    return () => {
      mounted = false;
      if (unlistenRecording) {
        unlistenRecording();
      } else {
        recordingPromise.then((fn) => fn());
      }
      if (unlistenTranscript) {
        unlistenTranscript();
      } else {
        transcriptPromise.then((fn) => fn());
      }
      if (unlistenError) {
        unlistenError();
      } else {
        errorPromise.then((fn) => fn());
      }
      if (unlistenChatState) {
        unlistenChatState();
      } else {
        chatStatePromise.then((fn) => fn());
      }
      if (unlistenChatDelta) {
        unlistenChatDelta();
      } else {
        chatDeltaPromise.then((fn) => fn());
      }
      if (unlistenChatFinished) {
        unlistenChatFinished();
      } else {
        chatFinishedPromise.then((fn) => fn());
      }
      if (unlistenTtsState) {
        unlistenTtsState();
      } else {
        ttsStatePromise.then((fn) => fn());
      }
      if (unlistenToolEvent) {
        unlistenToolEvent();
      } else {
        toolEventPromise.then((fn) => fn());
      }
    };
  }, [
    appendStreamingText,
    clearError,
    currentSessionId,
    fetchSession,
    pushLiveToolActivity,
    resetStreamingText,
    resetLiveToolActivities,
    setError,
    setLastTranscript,
    setPhase,
    setRecording,
    setSpeaking,
    setStreaming,
  ]);

  useEffect(() => {
    if (!config) {
      return;
    }

    if (config.mode === "voice_command" && activeNav === "ai") {
      setActiveNav("macros");
    }
  }, [activeNav, config]);

  if (isLoading || !config) {
    return (
      <div className="h-screen w-screen flex items-center justify-center bg-[#0F1115]">
        <Loader2 className="w-8 h-8 text-[#FCE100] animate-spin" />
      </div>
    );
  }

  return (
    <div className="h-screen w-screen flex flex-col overflow-hidden rounded-lg border border-zinc-800 bg-[#0F1115]">
      <CustomTitlebar />
      <Toaster
        theme="dark"
        richColors
        position="bottom-left"
        toastOptions={toasterOptions}
      />
      <UpdaterDialog />

      <div className="flex-1 flex overflow-hidden">
        <Sidebar activeNav={activeNav} setActiveNav={setActiveNav} />

        {/* Main Content Area */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Views */}
          {activeNav === "ai" && <AIView />}
          {activeNav === "macros" && <MacrosView />}
          {activeNav === "stratagems" && <StratagemsView />}
          {activeNav === "settings" && <GlobalSettingsView />}
          {activeNav === "keybindings" && <KeyBindingsView />}
          {activeNav === "log" && <LogView />}
        </div>
      </div>
    </div>
  );
}
