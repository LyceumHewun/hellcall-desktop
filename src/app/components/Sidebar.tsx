import { useEffect, useMemo, useState } from "react";
import {
  Bot,
  Settings,
  Keyboard,
  Command,
  Terminal,
  SatelliteDish,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useConfigStore } from "../../store/configStore";
import { useEngineStore } from "../../store/engineStore";
import {
  buildEngineStartSnapshot,
  createEngineStartSignature,
} from "../../store/engineConfig";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";

interface SidebarProps {
  activeNav: string;
  setActiveNav: (nav: string) => void;
}

export function Sidebar({ activeNav, setActiveNav }: SidebarProps) {
  const { t } = useTranslation();
  const config = useConfigStore((state) => state.config);
  const {
    status,
    setStatus,
    aiStatus,
    setAiStatus,
    aiWarmupStage,
    setAiWarmupStage,
    lastStartedConfigSignature,
    setLastStartedConfigSignature,
    selectedDevice,
    selectedVoskModelId,
    selectedVoskModelReady,
    setSelectedVoskModelReady,
    selectedVisionModelId,
    setSelectedVisionModelReady,
  } = useEngineStore();
  const [aiRuntimeReady, setAiRuntimeReady] = useState<boolean | null>(null);
  const [aiSttReady, setAiSttReady] = useState<boolean | null>(null);
  const [aiTtsReady, setAiTtsReady] = useState<boolean | null>(null);

  const currentEngineConfigSignature = config
    ? createEngineStartSignature(
        buildEngineStartSnapshot(
          config,
          selectedDevice,
          selectedVoskModelId,
          selectedVisionModelId,
        ),
      )
    : null;

  useEffect(() => {
    let cancelled = false;

    const syncSelectedModelState = async () => {
      try {
        const [voskModels, visionModels] = await Promise.all([
          invoke<Array<{ id: string; is_downloaded: boolean }>>(
            "get_available_vosk_models",
          ),
          invoke<Array<{ id: string; is_downloaded: boolean }>>(
            "get_available_vision_models",
          ),
        ]);
        if (cancelled) {
          return;
        }

        const selectedVoskModel = voskModels.find(
          (model) => model.id === selectedVoskModelId,
        );
        const selectedVisionModel = visionModels.find(
          (model) => model.id === selectedVisionModelId,
        );
        setSelectedVoskModelReady(Boolean(selectedVoskModel?.is_downloaded));
        setSelectedVisionModelReady(Boolean(selectedVisionModel?.is_downloaded));
      } catch (error) {
        if (!cancelled) {
          console.error("Failed to load model status:", error);
          setSelectedVoskModelReady(false);
          setSelectedVisionModelReady(null);
        }
      }
    };

    syncSelectedModelState();

    return () => {
      cancelled = true;
    };
  }, [
    selectedVisionModelId,
    selectedVoskModelId,
    setSelectedVisionModelReady,
    setSelectedVoskModelReady,
  ]);

  useEffect(() => {
    if (!config || config.mode !== "ai_agent") {
      setAiRuntimeReady(null);
      setAiSttReady(null);
      setAiTtsReady(null);
      return;
    }

    let cancelled = false;
    let unlistenRuntime: UnlistenFn | null = null;
    let unlistenStt: UnlistenFn | null = null;
    let unlistenTts: UnlistenFn | null = null;

    const refreshAiSpeechAssetStatus = async () => {
      try {
        const [runtimePackages, sttModels, ttsModels] = await Promise.all([
          invoke<Array<{ id: string; is_downloaded: boolean }>>(
            "get_available_sherpa_runtime",
          ),
          invoke<Array<{ id: string; is_downloaded: boolean }>>(
            "get_available_sherpa_stt_models",
          ),
          invoke<Array<{ id: string; is_downloaded: boolean }>>(
            "get_available_sherpa_tts_models",
          ),
        ]);

        if (cancelled) {
          return;
        }

        const runtimePackage = runtimePackages.find(
          (item) => item.id === "sherpa-onnx-v1.12.9-win-x64-shared",
        );
        const sttModel = sttModels.find(
          (item) => item.id === config.ai.speech.stt.model_id,
        );
        const ttsModel = ttsModels.find(
          (item) => item.id === config.ai.speech.tts.model_id,
        );

        setAiRuntimeReady(Boolean(runtimePackage?.is_downloaded));
        setAiSttReady(Boolean(sttModel?.is_downloaded));
        setAiTtsReady(Boolean(ttsModel?.is_downloaded));
      } catch (error) {
        if (!cancelled) {
          console.error("Failed to load AI speech asset status:", error);
          setAiRuntimeReady(false);
          setAiSttReady(false);
          setAiTtsReady(false);
        }
      }
    };

    void refreshAiSpeechAssetStatus();

    const runtimePromise = listen<{ status: string }>(
      "sherpa-runtime-download-progress",
      (event) => {
        if (
          event.payload.status === "Complete" ||
          event.payload.status.startsWith("Failed:")
        ) {
          void refreshAiSpeechAssetStatus();
        }
      },
    ).then((fn) => {
      unlistenRuntime = fn;
      return fn;
    });

    const sttPromise = listen<{ status: string }>(
      "sherpa-stt-download-progress",
      (event) => {
        if (
          event.payload.status === "Complete" ||
          event.payload.status.startsWith("Failed:")
        ) {
          void refreshAiSpeechAssetStatus();
        }
      },
    ).then((fn) => {
      unlistenStt = fn;
      return fn;
    });

    const ttsPromise = listen<{ status: string }>(
      "sherpa-tts-download-progress",
      (event) => {
        if (
          event.payload.status === "Complete" ||
          event.payload.status.startsWith("Failed:")
        ) {
          void refreshAiSpeechAssetStatus();
        }
      },
    ).then((fn) => {
      unlistenTts = fn;
      return fn;
    });

    return () => {
      cancelled = true;
      if (unlistenRuntime) {
        unlistenRuntime();
      } else {
        runtimePromise.then((fn) => fn());
      }
      if (unlistenStt) {
        unlistenStt();
      } else {
        sttPromise.then((fn) => fn());
      }
      if (unlistenTts) {
        unlistenTts();
      } else {
        ttsPromise.then((fn) => fn());
      }
    };
  }, [config]);

  useEffect(() => {
    let unlistenWarmup: UnlistenFn | null = null;
    let unlistenWarmupError: UnlistenFn | null = null;

    const warmupPromise = listen<{ stage: string }>("ai-warmup-state", (event) => {
      const stage = event.payload.stage;
      if (stage === "READY") {
        setAiWarmupStage(null);
        setAiStatus("READY");
        return;
      }

      if (stage === "OFFLINE") {
        setAiWarmupStage(null);
        setAiStatus("OFFLINE");
        return;
      }

      setAiWarmupStage(
        stage === "LOADING_RUNTIME" ||
          stage === "LOADING_STT" ||
          stage === "LOADING_TTS"
          ? stage
          : null,
      );
      setAiStatus("WARMING_UP");
    }).then((fn) => {
      unlistenWarmup = fn;
      return fn;
    });

    const warmupErrorPromise = listen<{ message: string }>("ai-agent-error", (event) => {
      setAiWarmupStage(null);
      setAiStatus("OFFLINE");
      toast.error(event.payload.message);
    }).then((fn) => {
      unlistenWarmupError = fn;
      return fn;
    });

    return () => {
      if (unlistenWarmup) {
        unlistenWarmup();
      } else {
        warmupPromise.then((fn) => fn());
      }
      if (unlistenWarmupError) {
        unlistenWarmupError();
      } else {
        warmupErrorPromise.then((fn) => fn());
      }
    };
  }, [setAiStatus, setAiWarmupStage]);

  const toggleEngine = async () => {
    if (status === "OFFLINE") {
      const state = useConfigStore.getState();
      if (!state.config) return;

      // Validate that there is at least one valid macro before starting
      const validMacrosCount = state.config.commands.filter(
        (c) => c.command.trim() !== "" && c.keys.length > 0,
      ).length;
      if (validMacrosCount === 0) {
        return;
      }

      if (selectedVoskModelReady === false) {
        return;
      }

      setStatus("STARTING");
      try {
        if (!state.config) throw new Error("Config not loaded");

        const engineStartSnapshot = buildEngineStartSnapshot(
          state.config,
          selectedDevice,
          selectedVoskModelId,
          selectedVisionModelId,
        );

        await invoke("start_engine", {
          config: engineStartSnapshot.config,
          deviceName: engineStartSnapshot.selectedDevice,
          selectedModelId: engineStartSnapshot.selectedVoskModelId,
          selectedVisionModelId: engineStartSnapshot.selectedVisionModelId,
        });
        setLastStartedConfigSignature(
          createEngineStartSignature(engineStartSnapshot),
        );
        setStatus("ACTIVE");
      } catch (error) {
        console.error("Failed to start engine:", error);
        setStatus("OFFLINE");
      }
    } else if (status === "ACTIVE") {
      try {
        await invoke("stop_engine");
        setStatus("OFFLINE");
      } catch (error) {
        console.error("Failed to stop engine:", error);
      }
    }
  };

  const toggleAiAgent = async () => {
    if (!config || config.mode !== "ai_agent") {
      return;
    }

    if (aiStatus === "OFFLINE") {
      setAiWarmupStage("LOADING_RUNTIME");
      setAiStatus("WARMING_UP");
      try {
        await invoke("start_ai_agent");
      } catch (error) {
        console.error("Failed to start AI agent:", error);
        toast.error(error instanceof Error ? error.message : String(error));
        setAiWarmupStage(null);
        setAiStatus("OFFLINE");
      }
      return;
    }

    if (aiStatus === "READY") {
      try {
        await invoke("stop_ai_agent");
      } catch (error) {
        console.error("Failed to stop AI agent:", error);
      } finally {
        setAiWarmupStage(null);
        setAiStatus("OFFLINE");
      }
    }
  };

  const isActive = status === "ACTIVE";
  const isStarting = status === "STARTING";
  const isAiMode = config?.mode === "ai_agent";
  const isAiReady = aiStatus === "READY";
  const isAiWarmingUp = aiStatus === "WARMING_UP";
  const aiWarmupLabel = (() => {
    switch (aiWarmupStage) {
      case "LOADING_RUNTIME":
        return t("status.ai_loading_runtime");
      case "LOADING_STT":
        return t("status.ai_loading_stt");
      case "LOADING_TTS":
        return t("status.ai_loading_tts");
      default:
        return t("status.ai_warming_up");
    }
  })();
  const validMacrosCount =
    config?.commands.filter(
      (commandConfig) =>
        commandConfig.command.trim() !== "" && commandConfig.keys.length > 0,
    ).length ?? 0;
  const showCommandRequiredHint = validMacrosCount === 0;
  const isStartDisabled =
    isStarting ||
    (!isActive && selectedVoskModelReady === false) ||
    (!isActive && showCommandRequiredHint);
  const showRestartReminder =
    isActive &&
    currentEngineConfigSignature !== null &&
    lastStartedConfigSignature !== null &&
    currentEngineConfigSignature !== lastStartedConfigSignature;
  const aiSidebarHints = useMemo(() => {
    if (!isAiMode) {
      return [];
    }

    if (aiRuntimeReady === false) {
      return [t("ai.models.runtime_required")];
    }
    if (aiSttReady === false) {
      return [t("ai.models.stt_required")];
    }
    if (config?.ai.speech.tts.enabled && aiTtsReady === false) {
      return [t("ai.models.tts_required")];
    }
    return [];
  }, [aiRuntimeReady, aiSttReady, aiTtsReady, config, isAiMode, t]);

  return (
    <div className="w-64 bg-[#0F1115] border-r border-white/10 flex flex-col p-4 gap-6">
      {isAiMode ? (
        <button
          onClick={toggleAiAgent}
          disabled={isAiWarmingUp}
          className={`relative overflow-hidden transition-all duration-300 cursor-pointer border-2 rounded p-4 group disabled:opacity-80 disabled:hover:scale-100 disabled:cursor-not-allowed ${
            isAiReady || isAiWarmingUp
              ? "bg-[#FCE100] border-[#FCE100] shadow-[0_0_20px_rgba(252,225,0,0.3)]"
              : "bg-transparent border-white/20 hover:border-[#FCE100]/50 hover:bg-white/5 active:scale-[0.98]"
          } ${isAiWarmingUp ? "animate-pulse" : ""}`}
        >
          <div
            className={`absolute inset-0 transition-opacity duration-1000 ${
              isAiReady || isAiWarmingUp ? "opacity-100" : "opacity-0"
            }`}
            style={{
              background:
                "radial-gradient(circle at center, rgba(252, 225, 0, 0.2) 0%, transparent 70%)",
              animation: isAiReady ? "pulse 2s ease-in-out infinite" : "none",
            }}
          />
          <div className="relative flex flex-col items-center gap-2 text-center">
            <div className="flex h-3 items-center justify-center overflow-visible">
              <Bot
                className={`h-5 w-5 ${
                  isAiReady || isAiWarmingUp ? "text-black" : "text-white/70"
                }`}
              />
            </div>
            <span
              style={{ fontFamily: "var(--font-family-tech)" }}
              className={`tracking-wider ${
                isAiReady || isAiWarmingUp ? "text-black" : "text-white/70"
              }`}
            >
              {t("status.ai_agent")}
            </span>
            <span
              style={{ fontFamily: "var(--font-family-tech)" }}
              className={`tracking-wider ${
                isAiReady || isAiWarmingUp ? "text-black" : "text-white/50"
              }`}
            >
              {isAiWarmingUp
                ? aiWarmupLabel
                : isAiReady
                  ? t("status.ai_ready")
                  : t("status.ai_offline")}
            </span>
          </div>
        </button>
      ) : (
        <button
          onClick={toggleEngine}
          disabled={isStartDisabled}
          className={`relative overflow-hidden transition-all duration-300 cursor-pointer border-2 rounded p-4 group disabled:opacity-80 disabled:hover:scale-100 disabled:cursor-not-allowed ${
            isActive || isStarting
              ? "bg-[#FCE100] border-[#FCE100] shadow-[0_0_20px_rgba(252,225,0,0.3)]"
              : "bg-transparent border-white/20 hover:border-[#FCE100]/50 hover:bg-white/5 active:scale-[0.98]"
          } ${isStarting ? "animate-pulse" : ""}`}
        >
          <div
            className={`absolute inset-0 transition-opacity duration-1000 ${
              isActive || isStarting ? "opacity-100" : "opacity-0"
            }`}
            style={{
              background:
                "radial-gradient(circle at center, rgba(252, 225, 0, 0.2) 0%, transparent 70%)",
              animation: isActive ? "pulse 2s ease-in-out infinite" : "none",
            }}
          />
          <div className="relative flex flex-col items-center gap-2">
            <div
              className={`w-3 h-3 rounded-full transition-all duration-300 ${
                isActive || isStarting
                  ? "bg-black"
                  : "bg-[#FCE100]/50 group-hover:bg-[#FCE100] group-hover:shadow-[0_0_10px_rgba(252,225,0,0.5)]"
              }`}
            />
            <span
              style={{ fontFamily: "var(--font-family-tech)" }}
              className={`tracking-wider ${
                isActive || isStarting ? "text-black" : "text-white/70"
              }`}
            >
              {t("status.voice_link")}
            </span>
            <span
              style={{ fontFamily: "var(--font-family-tech)" }}
              className={`tracking-wider ${
                isActive || isStarting ? "text-black" : "text-white/50"
              }`}
            >
              {isStarting
                ? t("status.linking")
                : isActive
                  ? t("status.active")
                  : t("status.offline")}
            </span>
          </div>
        </button>
      )}

      {/* Navigation */}
      <nav className="flex flex-col gap-1">
        {isAiMode ? (
          <button
            onClick={() => setActiveNav("ai")}
            className={`flex items-center gap-3 px-4 py-3 rounded transition-colors ${
              activeNav === "ai"
                ? "bg-white/10 text-white"
                : "text-white/60 hover:bg-white/5 hover:text-white/80"
            }`}
          >
            <Bot className="w-4 h-4" />
            <span>{t("nav.ai")}</span>
          </button>
        ) : null}

        <button
          onClick={() => setActiveNav("macros")}
          className={`flex items-center gap-3 px-4 py-3 rounded transition-colors ${
            activeNav === "macros"
              ? "bg-white/10 text-white"
              : "text-white/60 hover:bg-white/5 hover:text-white/80"
          }`}
        >
          <Command className="w-4 h-4" />
          <span>{t("nav.macros")}</span>
        </button>

        <button
          onClick={() => setActiveNav("keybindings")}
          className={`flex items-center gap-3 px-4 py-3 rounded transition-colors ${
            activeNav === "keybindings"
              ? "bg-white/10 text-white"
              : "text-white/60 hover:bg-white/5 hover:text-white/80"
          }`}
        >
          <Keyboard className="w-4 h-4" />
          <span>{t("nav.keybindings")}</span>
        </button>

        <button
          onClick={() => setActiveNav("stratagems")}
          className={`flex items-center gap-3 px-4 py-3 rounded transition-colors ${
            activeNav === "stratagems"
              ? "bg-white/10 text-white"
              : "text-white/60 hover:bg-white/5 hover:text-white/80"
          }`}
        >
          <SatelliteDish className="w-4 h-4" />
          <span>{t("nav.stratagems")}</span>
        </button>

        <button
          onClick={() => setActiveNav("log")}
          className={`flex items-center gap-3 px-4 py-3 rounded transition-colors ${
            activeNav === "log"
              ? "bg-white/10 text-white"
              : "text-white/60 hover:bg-white/5 hover:text-white/80"
          }`}
        >
          <Terminal className="w-4 h-4" />
          <span>{t("nav.log")}</span>
        </button>

        <button
          onClick={() => setActiveNav("settings")}
          className={`flex items-center gap-3 px-4 py-3 rounded transition-colors ${
            activeNav === "settings"
              ? "bg-white/10 text-white"
              : "text-white/60 hover:bg-white/5 hover:text-white/80"
          }`}
        >
          <Settings className="w-4 h-4" />
          <span>{t("nav.settings")}</span>
        </button>
      </nav>

      <div className="mt-auto flex flex-col gap-2">
        {showRestartReminder ? (
          <div className="rounded-md border border-amber-400/30 bg-amber-400/10 px-3 py-2 text-xs text-amber-100/90">
            {t("status.restart_required")}
          </div>
        ) : null}

        {selectedVoskModelReady === false && !isActive && !isAiMode ? (
          <div className="rounded-md border border-amber-400/30 bg-amber-400/10 px-3 py-2 text-xs text-amber-100/90">
            {t("settings.model_required")}
          </div>
        ) : null}

        {showCommandRequiredHint && !isAiMode ? (
          <div className="rounded-md border border-amber-400/30 bg-amber-400/10 px-3 py-2 text-xs text-amber-100/90">
            {t("status.command_required")}
          </div>
        ) : null}

        {aiSidebarHints.map((hint) => (
          <div
            key={hint}
            className="rounded-md border border-amber-400/30 bg-amber-400/10 px-3 py-2 text-xs text-amber-100/90"
          >
            {hint}
          </div>
        ))}
      </div>

      <style>{`
        @keyframes pulse {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.6; }
        }
      `}</style>
    </div>
  );
}
