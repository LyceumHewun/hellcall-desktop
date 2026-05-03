import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Plus, Trash2, Waves } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useConfigStore } from "../../store/configStore";
import { useAiStore } from "../../store/aiStore";
import { useEngineStore } from "../../store/engineStore";
import {
  AiDebugLogPayload,
  AiDebugStage,
  AiLiveToolActivity,
  AiSessionEvent,
} from "../../types/ai";
import { AssetModelSelector } from "../components/AssetModelSelector";
import { Button } from "../components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { Input } from "../components/ui/input";
import { Label } from "../components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../components/ui/select";
import { Switch } from "../components/ui/switch";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "../components/ui/tabs";
import { Textarea } from "../components/ui/textarea";

const AVAILABLE_SKILLS = [
  "send_key_sequence",
  "execute_stratagem",
  "list_stratagems",
  "get_key_mappings",
];

const NOOP = (..._args: unknown[]) => undefined;
const SHERPA_RUNTIME_ID = "sherpa-onnx-v1.12.9-win-x64-shared";
const CONTEXT_EVENT_COUNT_OPTIONS = [4, 8, 12, 20, 50];

function formatTimestamp(value: number) {
  return new Date(value).toLocaleString();
}

function formatStructuredText(value: string | null | undefined) {
  if (!value) {
    return "";
  }

  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
}

function formatDebugDetail(value: unknown) {
  if (value === null || value === undefined) {
    return "";
  }

  if (typeof value === "string") {
    return formatStructuredText(value);
  }

  return JSON.stringify(value, null, 2);
}

function parseToolCalls(event: AiSessionEvent) {
  if (event.kind !== "assistant_tool_calls" || !event.text) {
    return [];
  }

  try {
    return JSON.parse(event.text) as Array<{
      id?: string;
      function?: { name?: string; arguments?: string };
    }>;
  } catch {
    return [];
  }
}

function parseToolResult(event: AiSessionEvent) {
  if (event.kind !== "tool_result" || !event.text) {
    return null;
  }

  try {
    return JSON.parse(event.text) as {
      tool_call_id?: string;
      name?: string;
      content?: string;
    };
  } catch {
    return null;
  }
}

function renderEventSummary(event: AiSessionEvent) {
  if (!event.text) {
    return "";
  }

  if (event.kind === "assistant_tool_calls") {
    try {
      const parsed = JSON.parse(event.text) as Array<{
        function?: { name?: string; arguments?: string };
      }>;
      return parsed
        .map((item) => item.function?.name || "tool")
        .filter(Boolean)
        .join(", ");
    } catch {
      return event.text;
    }
  }

  if (event.kind === "tool_result") {
    try {
      const parsed = JSON.parse(event.text) as {
        name?: string;
        content?: string;
      };
      return parsed.content || parsed.name || event.text;
    } catch {
      return event.text;
    }
  }

  return event.text;
}

function eventTitle(kind: string, t: (key: string) => string) {
  switch (kind) {
    case "user_transcript":
      return t("ai.events.user");
    case "assistant_final":
      return t("ai.events.assistant");
    case "assistant_partial":
      return t("ai.events.assistant_partial");
    case "assistant_tool_calls":
      return t("ai.events.tool_call");
    case "tool_result":
      return t("ai.events.tool_result");
    default:
      return kind;
  }
}

function eventClasses(kind: string) {
  switch (kind) {
    case "user_transcript":
      return "border-sky-400/25 bg-sky-400/8";
    case "assistant_final":
      return "border-white/10 bg-black/20";
    case "assistant_partial":
      return "border-white/10 bg-black/20";
    case "assistant_tool_calls":
      return "border-[#FCE100]/30 bg-[#FCE100]/10";
    case "tool_result":
      return "border-emerald-400/25 bg-emerald-400/8";
    default:
      return "border-white/10 bg-black/20";
  }
}

function toolPhaseClasses(phase: AiLiveToolActivity["phase"]) {
  switch (phase) {
    case "call":
      return "border-[#FCE100]/30 bg-[#FCE100]/10";
    case "result":
      return "border-emerald-400/25 bg-emerald-400/8";
    case "error":
      return "border-red-500/30 bg-red-500/10";
    default:
      return "border-white/10 bg-black/20";
  }
}

function debugLevelClasses(level: AiDebugLogPayload["level"]) {
  switch (level) {
    case "success":
      return "border-emerald-400/25 bg-emerald-400/8 text-emerald-100";
    case "warn":
      return "border-amber-400/30 bg-amber-400/10 text-amber-100";
    case "error":
      return "border-red-500/30 bg-red-500/10 text-red-100";
    default:
      return "border-white/10 bg-black/20 text-white/80";
  }
}

function toolPhaseLabel(
  phase: AiLiveToolActivity["phase"],
  t: (key: string) => string,
) {
  switch (phase) {
    case "call":
      return t("ai.tool_phases.call");
    case "result":
      return t("ai.tool_phases.result");
    case "error":
      return t("ai.tool_phases.error");
    default:
      return phase;
  }
}

export function AIView() {
  const { t } = useTranslation();
  const { config, updateConfig } = useConfigStore();
  const aiStatus = useEngineStore((state) => state.aiStatus);
  const conversationContainerRef = useRef<HTMLDivElement | null>(null);
  const [runtimeReady, setRuntimeReady] = useState<boolean | null>(null);
  const [sttReady, setSttReady] = useState<boolean | null>(null);
  const [ttsReady, setTtsReady] = useState<boolean | null>(null);
  const {
    currentSession,
    liveToolActivities,
    isRecording,
    isStreaming,
    isSpeaking,
    phase,
    streamingText,
    isLoadingSession,
    error,
    setError,
    clearError,
    resetStreamingText,
    resetLiveToolActivities,
    setLastTranscript,
    fetchSession,
  } = useAiStore();
  const isManualHoldRef = useRef(false);
  const [debugEnabled, setDebugEnabled] = useState(false);
  const [debugActiveTab, setDebugActiveTab] = useState<AiDebugStage>("stt");
  const [debugRunningStage, setDebugRunningStage] =
    useState<AiDebugStage | null>(null);
  const [debugLogs, setDebugLogs] = useState<AiDebugLogPayload[]>([]);
  const [llmDebugInput, setLlmDebugInput] = useState("");
  const [ttsDebugInput, setTtsDebugInput] = useState("");

  useEffect(() => {
    fetchSession();
  }, [fetchSession]);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    const promise = listen<AiDebugLogPayload>("ai-debug-log", (event) => {
      setDebugLogs((logs) => [...logs, event.payload]);
      if (
        event.payload.message === "test finished" ||
        event.payload.message === "test stopped" ||
        event.payload.level === "error"
      ) {
        setDebugRunningStage((stage) =>
          stage === event.payload.stage ? null : stage,
        );
      }
    }).then((fn) => {
      unlisten = fn;
      return fn;
    });

    return () => {
      if (unlisten) {
        unlisten();
      } else {
        promise.then((fn) => fn());
      }
    };
  }, []);

  useEffect(() => {
    if (!config || config.mode !== "ai_agent") {
      setRuntimeReady(null);
      setSttReady(null);
      setTtsReady(null);
      return;
    }

    let cancelled = false;

    const refreshSpeechAssetStatus = async () => {
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
          (item) => item.id === SHERPA_RUNTIME_ID,
        );
        const sttModel = sttModels.find(
          (item) => item.id === config.ai.speech.stt.model_id,
        );
        const ttsModel = ttsModels.find(
          (item) => item.id === config.ai.speech.tts.model_id,
        );

        setRuntimeReady(Boolean(runtimePackage?.is_downloaded));
        setSttReady(Boolean(sttModel?.is_downloaded));
        setTtsReady(Boolean(ttsModel?.is_downloaded));
      } catch (statusError) {
        if (cancelled) {
          return;
        }

        console.error("Failed to load sherpa asset status:", statusError);
        setRuntimeReady(false);
        setSttReady(false);
        setTtsReady(false);
      }
    };

    void refreshSpeechAssetStatus();

    let unlistenRuntime: UnlistenFn | null = null;
    let unlistenStt: UnlistenFn | null = null;
    let unlistenTts: UnlistenFn | null = null;

    const runtimePromise = listen<{ status: string }>(
      "sherpa-runtime-download-progress",
      (event) => {
        if (
          event.payload.status === "Complete" ||
          event.payload.status.startsWith("Failed:")
        ) {
          void refreshSpeechAssetStatus();
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
          void refreshSpeechAssetStatus();
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
          void refreshSpeechAssetStatus();
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

  const startManualRecording = async () => {
    if (isManualHoldRef.current || isRecording || isStreaming || isSpeaking) {
      return;
    }

    isManualHoldRef.current = true;
    setLastTranscript(null);

    try {
      clearError();
      resetStreamingText();
      resetLiveToolActivities();
      await invoke("start_ai_recording");
    } catch (recordingError) {
      const message =
        recordingError instanceof Error
          ? recordingError.message
          : String(recordingError ?? t("ai.errors.start_recording"));
      console.error("Failed to start AI recording:", recordingError);
      setError(message);
      toast.error(message);
      isManualHoldRef.current = false;
    }
  };

  const stopManualRecording = async () => {
    if (!isManualHoldRef.current) {
      return;
    }

    isManualHoldRef.current = false;
    try {
      clearError();
      await invoke("stop_ai_recording");
    } catch (recordingError) {
      const message =
        recordingError instanceof Error
          ? recordingError.message
          : String(recordingError ?? t("ai.errors.stop_recording"));
      console.error("Failed to stop AI recording:", recordingError);
      setError(message);
      toast.error(message);
    }
  };

  const stopStreaming = async () => {
    try {
      await invoke("stop_ai_chat_stream");
    } catch (streamError) {
      const message =
        streamError instanceof Error
          ? streamError.message
          : String(streamError ?? t("ai.errors.start_chat"));
      setError(message);
      toast.error(message);
    }
  };

  const appendDebugLog = (
    stage: AiDebugStage,
    level: AiDebugLogPayload["level"],
    message: string,
    detail?: unknown,
  ) => {
    setDebugLogs((logs) => [
      ...logs,
      {
        id: `local-debug-${Date.now()}-${logs.length}`,
        stage,
        level,
        message,
        detail,
        created_at_ms: Date.now(),
      },
    ]);
  };

  const validateDebugPreconditions = (stage: AiDebugStage) => {
    if (!config) {
      appendDebugLog(stage, "error", t("ai.debug.errors.config_missing"));
      return false;
    }

    if (aiStatus !== "READY") {
      appendDebugLog(stage, "error", t("ai.debug.errors.agent_not_ready"));
      return false;
    }

    if (stage === "stt") {
      if (runtimeReady === false || sttReady === false) {
        appendDebugLog(stage, "error", t("ai.debug.errors.stt_not_ready"));
        return false;
      }
      return true;
    }

    if (stage === "llm") {
      if (!config.ai.llm.enabled) {
        appendDebugLog(stage, "error", t("ai.debug.errors.llm_disabled"));
        return false;
      }
      if (!config.ai.llm.decision.api_key.trim()) {
        appendDebugLog(stage, "error", t("ai.debug.errors.decision_key_missing"));
        return false;
      }
      if (config.ai.llm.reply_enabled && !config.ai.llm.reply.api_key.trim()) {
        appendDebugLog(stage, "error", t("ai.debug.errors.reply_key_missing"));
        return false;
      }
      return true;
    }

    if (!config.ai.speech.tts.enabled) {
      appendDebugLog(stage, "error", t("ai.debug.errors.tts_disabled"));
      return false;
    }
    if (runtimeReady === false || ttsReady === false) {
      appendDebugLog(stage, "error", t("ai.debug.errors.tts_not_ready"));
      return false;
    }
    return true;
  };

  const toggleDebugEnabled = async (enabled: boolean) => {
    setDebugEnabled(enabled);
    if (!enabled && debugRunningStage) {
      try {
        await invoke("stop_ai_debug_test");
      } catch (stopError) {
        appendDebugLog(
          debugRunningStage,
          "error",
          stopError instanceof Error ? stopError.message : String(stopError),
        );
      } finally {
        setDebugRunningStage(null);
      }
    }
  };

  const toggleDebugSttTest = async () => {
    if (debugRunningStage === "stt") {
      try {
        await invoke("stop_ai_debug_stt_recording");
      } catch (debugError) {
        appendDebugLog(
          "stt",
          "error",
          debugError instanceof Error ? debugError.message : String(debugError),
        );
      } finally {
        setDebugRunningStage(null);
      }
      return;
    }
    if (debugRunningStage || !validateDebugPreconditions("stt")) {
      return;
    }

    try {
      await invoke("start_ai_debug_stt_recording");
      setDebugRunningStage("stt");
    } catch (debugError) {
      appendDebugLog(
        "stt",
        "error",
        debugError instanceof Error ? debugError.message : String(debugError),
      );
      setDebugRunningStage(null);
    }
  };

  const toggleDebugLlmTest = async () => {
    if (debugRunningStage === "llm") {
      try {
        await invoke("stop_ai_debug_test");
      } catch (debugError) {
        appendDebugLog(
          "llm",
          "error",
          debugError instanceof Error ? debugError.message : String(debugError),
        );
      } finally {
        setDebugRunningStage(null);
      }
      return;
    }
    if (debugRunningStage || !validateDebugPreconditions("llm")) {
      return;
    }
    if (!llmDebugInput.trim()) {
      appendDebugLog("llm", "error", t("ai.debug.errors.input_required"));
      return;
    }

    try {
      setDebugRunningStage("llm");
      await invoke("start_ai_debug_llm_test", { inputText: llmDebugInput });
    } catch (debugError) {
      appendDebugLog(
        "llm",
        "error",
        debugError instanceof Error ? debugError.message : String(debugError),
      );
      setDebugRunningStage(null);
    }
  };

  const toggleDebugTtsTest = async () => {
    if (debugRunningStage === "tts") {
      try {
        await invoke("stop_ai_debug_test");
      } catch (debugError) {
        appendDebugLog(
          "tts",
          "error",
          debugError instanceof Error ? debugError.message : String(debugError),
        );
      } finally {
        setDebugRunningStage(null);
      }
      return;
    }
    if (debugRunningStage || !validateDebugPreconditions("tts")) {
      return;
    }
    if (!ttsDebugInput.trim()) {
      appendDebugLog("tts", "error", t("ai.debug.errors.input_required"));
      return;
    }

    try {
      setDebugRunningStage("tts");
      await invoke("start_ai_debug_tts_test", { inputText: ttsDebugInput });
    } catch (debugError) {
      appendDebugLog(
        "tts",
        "error",
        debugError instanceof Error ? debugError.message : String(debugError),
      );
      setDebugRunningStage(null);
    }
  };

  const currentAgent = useMemo(() => {
    if (!config) {
      return null;
    }

    return (
      config.ai.agents.find((agent) => agent.id === config.ai.default_agent_id) ??
      config.ai.agents[0] ??
      null
    );
  }, [config]);

  const hasConversationContent =
    (currentSession?.events.length ?? 0) > 0 ||
    liveToolActivities.length > 0 ||
    Boolean(streamingText);
  const requiresSpeechDownload = runtimeReady === false || sttReady === false;
  const showTtsDownloadHint =
    Boolean(config?.ai.speech.tts.enabled) && ttsReady === false;
  const isAiReady = aiStatus === "READY";
  const isBusy = isStreaming || isSpeaking;
  const pttButtonLabel = isRecording
    ? t("ai.ptt_button_recording")
    : isSpeaking
      ? t("ai.ptt_button_speaking")
      : isStreaming
        ? phase === "tool_running"
          ? t("ai.ptt_button_tool_running")
          : t("ai.ptt_button_thinking")
    : aiStatus === "WARMING_UP"
      ? t("ai.ptt_button_warming_up")
      : aiStatus === "READY"
        ? t("ai.ptt_button")
        : t("ai.ptt_button_disabled");
  const pttDisabled = requiresSpeechDownload || !isAiReady || isBusy;
  const assistantPendingLabel =
    phase === "tool_running"
      ? t("ai.streaming_status_tool")
      : isSpeaking
        ? t("ai.streaming_status_speaking")
        : t("ai.streaming_status");
  const phaseBannerLabel = (() => {
    switch (phase) {
      case "listening":
        return t("status.ai_listening");
      case "transcribing":
        return t("status.ai_transcribing");
      case "thinking":
        return t("status.ai_thinking");
      case "tool_running":
        return t("status.ai_tool_running");
      case "speaking":
        return t("status.ai_speaking");
      case "error":
        return t("status.ai_error");
      default:
        return null;
    }
  })();

  useLayoutEffect(() => {
    const container = conversationContainerRef.current;
    if (!container) {
      return;
    }

    container.scrollTop = container.scrollHeight;
  }, []);

  useLayoutEffect(() => {
    const container = conversationContainerRef.current;
    if (!container) {
      return;
    }

    container.scrollTop = container.scrollHeight;
  }, [
    currentSession?.events.length,
    liveToolActivities.length,
    streamingText,
    isStreaming,
    error,
  ]);

  const updateCurrentAgent = (
    updater: (draft: NonNullable<typeof currentAgent>) => void,
  ) => {
    if (!currentAgent) {
      return;
    }

    updateConfig((draft) => {
      const target = draft.ai.agents.find((agent) => agent.id === currentAgent.id);
      if (!target) {
        return;
      }
      updater(target as NonNullable<typeof currentAgent>);
    });
  };

  const addAgent = () => {
    if (!config) {
      return;
    }

    const nextId = crypto.randomUUID();
    updateConfig((draft) => {
      draft.ai.agents.push({
        id: nextId,
        name: t("ai.agent.new_name"),
        description: t("ai.agent.new_description"),
        persona_prompt: t("ai.agent.new_persona"),
        chat_model: "",
        decision_chat_model: "",
        reply_chat_model: "",
        temperature: 0.7,
        max_tokens: 2048,
        enable_thinking: false,
        skill_ids: ["send_key_sequence", "list_stratagems"],
        is_builtin: false,
      });
      draft.ai.default_agent_id = nextId;
    });
  };

  const deleteCurrentAgent = () => {
    if (!config || !currentAgent || config.ai.agents.length <= 1) {
      return;
    }

    updateConfig((draft) => {
      draft.ai.agents = draft.ai.agents.filter((agent) => agent.id !== currentAgent.id);
      draft.ai.default_agent_id = draft.ai.agents[0]?.id ?? "";
    });
  };

  if (!config) {
    return null;
  }

  const renderLlmStageConfig = (
    stage: "decision" | "reply",
    title: string,
  ) => {
    const stageConfig = config.ai.llm[stage];

    return (
      <div className="space-y-4 rounded-lg border border-white/10 bg-black/20 p-3">
        <p className="text-sm font-medium text-white">{title}</p>

        <div className="space-y-2">
          <Label>{t("ai.models.provider_kind")}</Label>
          <Select
            value={stageConfig.kind}
            onValueChange={(value: "siliconflow" | "openai_compatible") =>
              updateConfig((draft) => {
                draft.ai.llm[stage].kind = value;
                if (value === "siliconflow") {
                  draft.ai.llm[stage].base_url = "https://api.siliconflow.cn/v1";
                }
              })
            }
          >
            <SelectTrigger className="bg-black/30 text-white">
              <SelectValue />
            </SelectTrigger>
            <SelectContent className="border-white/10 bg-[#1E2128] text-white">
              <SelectItem value="siliconflow">SiliconFlow</SelectItem>
              <SelectItem value="openai_compatible">
                {t("ai.models.custom_openai")}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        {stageConfig.kind === "openai_compatible" ? (
          <div className="space-y-2">
            <Label>{t("ai.models.base_url")}</Label>
            <Input
              className="bg-black/30 border-white/10"
              value={stageConfig.base_url}
              onChange={(event) =>
                updateConfig((draft) => {
                  draft.ai.llm[stage].base_url = event.target.value;
                })
              }
            />
          </div>
        ) : null}

        <div className="space-y-2">
          <Label>{t("ai.models.api_key")}</Label>
          <Input
            type="password"
            className="bg-black/30 border-white/10"
            value={stageConfig.api_key}
            onChange={(event) =>
              updateConfig((draft) => {
                draft.ai.llm[stage].api_key = event.target.value;
              })
            }
          />
        </div>

        <div className="space-y-2">
          <Label>{t("ai.models.chat_model")}</Label>
          <Input
            className="bg-black/30 border-white/10"
            value={stageConfig.chat_model}
            onChange={(event) =>
              updateConfig((draft) => {
                draft.ai.llm[stage].chat_model = event.target.value;
              })
            }
          />
        </div>
      </div>
    );
  };

  if (config.mode !== "ai_agent") {
    return (
      <div className="flex-1 overflow-y-auto p-6">
        <div className="mx-auto max-w-4xl">
          <Card className="border-white/10 bg-[#1E2128] text-white">
            <CardHeader>
              <CardTitle className="text-[#FCE100]">{t("ai.mode_locked_title")}</CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <p className="text-sm text-white/60">{t("ai.mode_locked_body")}</p>
              <Button
                className="cursor-pointer bg-[#FCE100] text-black hover:bg-[#FCE100]/90"
                onClick={() =>
                  updateConfig((draft) => {
                    draft.mode = "ai_agent";
                  })
                }
              >
                {t("ai.switch_mode")}
              </Button>
            </CardContent>
          </Card>
        </div>
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 bg-[#0C0E12]">
      <Tabs defaultValue="conversation" className="flex min-h-0 flex-1 flex-col px-4 pb-4 pt-3">
        <div className="pb-3">
          <TabsList className="grid w-full grid-cols-5 bg-white/5">
            <TabsTrigger value="conversation">{t("ai.tabs.conversation")}</TabsTrigger>
            <TabsTrigger value="agent">{t("ai.tabs.agent")}</TabsTrigger>
            <TabsTrigger value="skills">{t("ai.tabs.skills")}</TabsTrigger>
            <TabsTrigger value="models">{t("ai.tabs.models")}</TabsTrigger>
            <TabsTrigger value="debug">{t("ai.tabs.debug")}</TabsTrigger>
          </TabsList>
        </div>

        <TabsContent value="conversation" className="mt-0 min-h-0 flex-1">
          <div className="flex h-full flex-col rounded-2xl border border-white/10 bg-[#171A20]">
            <div ref={conversationContainerRef} className="flex-1 overflow-y-auto p-4">
                  {error ? (
                    <div className="mb-4 rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
                      {error}
                    </div>
                  ) : null}
                  {phaseBannerLabel ? (
                    <div className="mb-4 flex items-center gap-2 rounded-xl border border-white/10 bg-black/20 px-4 py-3 text-xs uppercase tracking-[0.18em] text-white/55">
                      <span className="inline-block h-2 w-2 rounded-full bg-[#FCE100]" />
                      <span>{phaseBannerLabel}</span>
                    </div>
                  ) : null}
                  {currentSession ? (
                    <>
                      {isLoadingSession ? (
                        <p className="text-sm text-white/50">{t("ai.loading_session")}</p>
                      ) : !hasConversationContent ? (
                        <div className="rounded-xl border border-dashed border-white/10 bg-black/20 p-6 text-center">
                          <Waves className="mx-auto mb-3 h-8 w-8 text-[#FCE100]/70" />
                          <p className="text-sm text-white/70">
                            {t("ai.empty_conversation_title")}
                          </p>
                          <p className="mt-2 text-xs text-white/45">
                            {t("ai.empty_conversation_body")}
                          </p>
                        </div>
                      ) : (
                        <div className="space-y-3">
                          {currentSession.events.map((event) => {
                            const toolCalls = parseToolCalls(event);
                            const toolResult = parseToolResult(event);

                            return (
                              <div
                                key={event.id}
                                className={`rounded-xl border p-4 ${eventClasses(event.kind)}`}
                              >
                                <div className="mb-3 flex items-center justify-between gap-3 text-xs text-white/40">
                                  <span className="truncate">
                                    {eventTitle(event.kind, t)}
                                  </span>
                                  <span className="shrink-0">
                                    {formatTimestamp(event.created_at_ms)}
                                  </span>
                                </div>

                                {toolCalls.length > 0 ? (
                                  <div className="space-y-2">
                                    {toolCalls.map((toolCall, index) => (
                                      <div
                                        key={toolCall.id ?? `${event.id}-${index}`}
                                        className="rounded-lg border border-white/10 bg-black/20 p-3"
                                      >
                                        <div className="mb-2 flex items-center justify-between gap-2">
                                          <span className="truncate text-sm font-medium text-white">
                                            {toolCall.function?.name ?? "tool"}
                                          </span>
                                          <span className="shrink-0 text-[10px] uppercase tracking-[0.18em] text-white/35">
                                            {t("ai.tool_phases.call")}
                                          </span>
                                        </div>
                                        {toolCall.function?.arguments ? (
                                          <pre className="overflow-x-auto whitespace-pre-wrap break-all rounded-lg border border-white/10 bg-black/30 p-3 text-xs text-white/65">
                                            {formatStructuredText(
                                              toolCall.function.arguments,
                                            )}
                                          </pre>
                                        ) : null}
                                      </div>
                                    ))}
                                  </div>
                                ) : toolResult ? (
                                  <div className="space-y-2">
                                    <div className="flex items-center justify-between gap-2">
                                      <span className="truncate text-sm font-medium text-white">
                                        {toolResult.name ?? t("ai.events.tool_result")}
                                      </span>
                                      <span className="shrink-0 text-[10px] uppercase tracking-[0.18em] text-white/35">
                                        {t("ai.tool_phases.result")}
                                      </span>
                                    </div>
                                    <pre className="overflow-x-auto whitespace-pre-wrap break-all rounded-lg border border-white/10 bg-black/20 p-3 text-xs text-white/70">
                                      {formatStructuredText(
                                        toolResult.content ?? event.text,
                                      ) || t("ai.empty_event")}
                                    </pre>
                                  </div>
                                ) : (
                                  <p className="whitespace-pre-wrap break-words text-sm text-white/75">
                                    {renderEventSummary(event) || t("ai.empty_event")}
                                  </p>
                                )}
                              </div>
                            );
                          })}

                          {liveToolActivities.length > 0 ? (
                            <div className="space-y-2">
                              <div className="px-1 text-[10px] uppercase tracking-[0.22em] text-white/35">
                                {t("ai.live_tools")}
                              </div>
                              {liveToolActivities.map((activity) => (
                                <div
                                  key={activity.id}
                                  className={`rounded-xl border p-4 ${toolPhaseClasses(
                                    activity.phase,
                                  )}`}
                                >
                                  <div className="mb-2 flex items-center justify-between gap-3">
                                    <span className="truncate text-sm font-medium text-white">
                                      {activity.name}
                                    </span>
                                    <span className="shrink-0 text-[10px] uppercase tracking-[0.18em] text-white/40">
                                      {toolPhaseLabel(activity.phase, t)}
                                    </span>
                                  </div>
                                  <p className="whitespace-pre-wrap break-words text-sm text-white/75">
                                    {activity.summary}
                                  </p>
                                </div>
                              ))}
                            </div>
                          ) : null}

                          {isStreaming || isSpeaking ? (
                            <div className="rounded-xl border border-[#FCE100]/30 bg-[#FCE100]/10 p-4">
                              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/45">
                                <span>{t("ai.streaming_label")}</span>
                                <div className="flex items-center gap-3">
                                  <span>{assistantPendingLabel}</span>
                                  {isStreaming ? (
                                    <Button
                                      variant="outline"
                                      className="h-7 border-white/15 bg-black/20 px-2 text-[10px] uppercase tracking-[0.14em] text-white/80 hover:bg-white/10"
                                      onClick={() => void stopStreaming()}
                                    >
                                      {t("ai.ptt_button_stop")}
                                    </Button>
                                  ) : null}
                                </div>
                              </div>
                              {streamingText ? (
                                <p className="whitespace-pre-wrap break-words text-sm text-white/85">
                                  {streamingText}
                                </p>
                              ) : (
                                <p className="text-sm text-white/55">
                                  {t("ai.streaming_placeholder")}
                                </p>
                              )}
                            </div>
                          ) : null}
                        </div>
                      )}
                    </>
                  ) : (
                    <div className="rounded-2xl border border-dashed border-white/10 bg-black/20 p-8 text-center">
                      <p className="text-sm text-white/60">{t("ai.select_or_create")}</p>
                    </div>
                  )}
            </div>

            <div className="border-t border-white/10 bg-[#12151A] p-4">
              <Button
                className={`w-full text-black ${
                  isRecording
                    ? "bg-[#FCE100] hover:bg-[#FCE100]/90"
                  : "bg-[#FCE100]/85 hover:bg-[#FCE100]"
                }`}
                disabled={pttDisabled}
                onMouseDown={() =>
                  isStreaming ? void stopStreaming() : void startManualRecording()
                }
                onMouseUp={() => void stopManualRecording()}
                onMouseLeave={() => void stopManualRecording()}
                onTouchStart={() =>
                  isStreaming ? void stopStreaming() : void startManualRecording()
                }
                onTouchEnd={() => void stopManualRecording()}
              >
                {isStreaming ? t("ai.ptt_button_stop") : pttButtonLabel}
              </Button>
            </div>
          </div>
        </TabsContent>

            <TabsContent value="agent" className="min-h-0 flex-1 overflow-y-auto p-4">
              <Card className="border-white/10 bg-[#1A1D24] text-white">
                <CardHeader>
                  <CardTitle className="text-[#FCE100]">{t("ai.agent.title")}</CardTitle>
                </CardHeader>
                <CardContent className="space-y-4">
                  <div className="flex gap-2">
                    <Select
                      value={config.ai.default_agent_id}
                      onValueChange={(value) =>
                        updateConfig((draft) => {
                          draft.ai.default_agent_id = value;
                        })
                      }
                    >
                      <SelectTrigger className="min-w-0 flex-1 bg-black/30 text-white">
                        <SelectValue placeholder={t("ai.agent.select")} />
                      </SelectTrigger>
                      <SelectContent className="border-white/10 bg-[#1E2128] text-white">
                        {config.ai.agents.map((agent) => (
                          <SelectItem key={agent.id} value={agent.id}>
                            {agent.name}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>

                    <Button
                      variant="outline"
                      className="cursor-pointer border-white/10 bg-black/20 hover:bg-white/10"
                      onClick={addAgent}
                    >
                      <Plus className="h-4 w-4" />
                    </Button>
                    <Button
                      variant="outline"
                      className="cursor-pointer border-white/10 bg-black/20 hover:bg-red-500/10 hover:text-red-300"
                      disabled={config.ai.agents.length <= 1}
                      onClick={deleteCurrentAgent}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>

                  {currentAgent ? (
                    <div className="space-y-4">
                      <div className="space-y-2">
                        <Label>{t("ai.agent.name")}</Label>
                        <Input
                          className="bg-black/30 border-white/10"
                          value={currentAgent.name}
                          onChange={(event) =>
                            updateCurrentAgent((draft) => {
                              draft.name = event.target.value;
                            })
                          }
                        />
                      </div>

                      <div className="space-y-2">
                        <Label>{t("ai.agent.description")}</Label>
                        <Input
                          className="bg-black/30 border-white/10"
                          value={currentAgent.description}
                          onChange={(event) =>
                            updateCurrentAgent((draft) => {
                              draft.description = event.target.value;
                            })
                          }
                        />
                      </div>

                      <div className="space-y-2">
                        <Label>{t("ai.agent.persona_prompt")}</Label>
                        <Textarea
                          className="min-h-40 bg-black/30 border-white/10"
                          value={currentAgent.persona_prompt}
                          onChange={(event) =>
                            updateCurrentAgent((draft) => {
                              draft.persona_prompt = event.target.value;
                            })
                          }
                        />
                      </div>

                      <div className="grid grid-cols-2 gap-3">
                        <div className="space-y-2">
                          <Label>{t("ai.agent.temperature")}</Label>
                          <Input
                            type="number"
                            min={0}
                            max={2}
                            step={0.1}
                            className="bg-black/30 border-white/10"
                            value={currentAgent.temperature}
                            onChange={(event) =>
                              updateCurrentAgent((draft) => {
                                draft.temperature = Number(event.target.value);
                              })
                            }
                          />
                        </div>
                        <div className="space-y-2">
                          <Label>{t("ai.agent.max_tokens")}</Label>
                          <Input
                            type="number"
                            min={256}
                            step={128}
                            className="bg-black/30 border-white/10"
                            value={currentAgent.max_tokens}
                            onChange={(event) =>
                              updateCurrentAgent((draft) => {
                                draft.max_tokens = Number(event.target.value);
                              })
                            }
                          />
                        </div>
                      </div>

                      <div className="space-y-3">
                        <div className="flex items-center justify-between">
                          <Label>{t("ai.agent.enable_thinking")}</Label>
                          <Switch
                            checked={currentAgent.enable_thinking}
                            onCheckedChange={(checked) =>
                              updateCurrentAgent((draft) => {
                                draft.enable_thinking = checked;
                              })
                            }
                          />
                        </div>
                        <p className="text-xs text-white/45">
                          {t("ai.agent.enable_thinking_hint")}
                        </p>
                      </div>
                    </div>
                  ) : null}
                </CardContent>
              </Card>
            </TabsContent>

            <TabsContent value="skills" className="min-h-0 flex-1 overflow-y-auto p-4">
              <Card className="border-white/10 bg-[#1A1D24] text-white">
                <CardHeader>
                  <CardTitle className="text-[#FCE100]">{t("ai.skills.title")}</CardTitle>
                </CardHeader>
                <CardContent className="space-y-3">
                  <div className="flex items-center justify-between gap-4">
                    <div>
                      <p className="text-sm text-white">{t("ai.skills.auto_execute")}</p>
                      <p className="text-xs text-white/45">
                        {t("ai.skills.auto_execute_hint")}
                      </p>
                    </div>
                    <Switch
                      checked={config.ai.auto_execute_skills}
                      onCheckedChange={(checked) =>
                        updateConfig((draft) => {
                          draft.ai.auto_execute_skills = checked;
                        })
                      }
                    />
                  </div>

                  <div className="space-y-2">
                    {AVAILABLE_SKILLS.map((skillId) => {
                      const enabled = currentAgent?.skill_ids.includes(skillId) ?? false;
                      return (
                        <label
                          key={skillId}
                          className="flex items-center justify-between gap-3 rounded-lg border border-white/10 bg-black/20 px-3 py-2"
                        >
                          <span className="min-w-0 break-all text-sm text-white/80">
                            {skillId}
                          </span>
                          <Switch
                            checked={enabled}
                            onCheckedChange={(checked) =>
                              updateCurrentAgent((draft) => {
                                const next = new Set(draft.skill_ids);
                                if (checked) {
                                  next.add(skillId);
                                } else {
                                  next.delete(skillId);
                                }
                                draft.skill_ids = [...next];
                              })
                            }
                          />
                        </label>
                      );
                    })}
                  </div>
                </CardContent>
              </Card>
            </TabsContent>

            <TabsContent value="models" className="min-h-0 flex-1 overflow-y-auto p-4">
              <div className="space-y-4">
                <Card className="border-white/10 bg-[#1A1D24] text-white">
                  <CardHeader>
                    <CardTitle className="text-[#FCE100]">
                      {t("ai.models.speech_title")}
                    </CardTitle>
                  </CardHeader>
                  <CardContent className="space-y-4">
                    <AssetModelSelector
                      label={t("ai.models.runtime_package")}
                      placeholder={t("ai.models.runtime_package")}
                      fetchCommand="get_available_sherpa_runtime"
                      downloadCommand="download_sherpa_runtime"
                      progressEventName="sherpa-runtime-download-progress"
                      selectedModelId={SHERPA_RUNTIME_ID}
                      setSelectedModelId={NOOP}
                      setSelectedModelReady={setRuntimeReady}
                      formatModelName={(id) => id}
                    />
                    <AssetModelSelector
                      label={t("ai.models.stt_model")}
                      placeholder={t("ai.models.stt_model")}
                      fetchCommand="get_available_sherpa_stt_models"
                      downloadCommand="download_sherpa_stt_model"
                      progressEventName="sherpa-stt-download-progress"
                      selectedModelId={config.ai.speech.stt.model_id}
                      setSelectedModelId={(modelId) =>
                        updateConfig((draft) => {
                          draft.ai.speech.stt.model_id = modelId;
                        })
                      }
                      setSelectedModelReady={setSttReady}
                      formatModelName={(id) => id}
                    />
                    <div className="space-y-2">
                      <Label>{t("ai.models.stt_language")}</Label>
                      <Input
                        className="bg-black/30 border-white/10"
                        value={config.ai.speech.stt.language}
                        onChange={(event) =>
                          updateConfig((draft) => {
                            draft.ai.speech.stt.language = event.target.value;
                          })
                        }
                      />
                    </div>
                    <div className="space-y-2">
                      <Label>{t("ai.models.stt_use_itn")}</Label>
                      <div className="flex items-center justify-between space-x-4">
                        <p className="text-sm text-white/50">
                          {t("ai.models.stt_use_itn_hint")}
                        </p>
                        <Switch
                          className="border cursor-pointer"
                          checked={config.ai.speech.stt.use_itn}
                          onCheckedChange={(checked) =>
                            updateConfig((draft) => {
                              draft.ai.speech.stt.use_itn = checked;
                            })
                          }
                        />
                      </div>
                    </div>
                    <div className="space-y-2">
                      <Label>{t("ai.models.tts_enabled")}</Label>
                      <div className="flex items-center justify-between space-x-4">
                        <p className="text-sm text-white/50">
                          {t("ai.models.tts_enabled_hint")}
                        </p>
                        <Switch
                          className="border cursor-pointer"
                          checked={config.ai.speech.tts.enabled}
                          onCheckedChange={(checked) =>
                            updateConfig((draft) => {
                              draft.ai.speech.tts.enabled = checked;
                            })
                          }
                        />
                      </div>
                    </div>
                    <AssetModelSelector
                      label={t("ai.models.tts_model")}
                      placeholder={t("ai.models.tts_model")}
                      fetchCommand="get_available_sherpa_tts_models"
                      downloadCommand="download_sherpa_tts_model"
                      progressEventName="sherpa-tts-download-progress"
                      selectedModelId={config.ai.speech.tts.model_id}
                      setSelectedModelId={(modelId) =>
                        updateConfig((draft) => {
                          draft.ai.speech.tts.model_id = modelId;
                        })
                      }
                      setSelectedModelReady={setTtsReady}
                      formatModelName={(id) => id}
                    />
                    <div className="grid grid-cols-2 gap-3">
                      <div className="space-y-2">
                        <Label>{t("ai.models.tts_speaker_id")}</Label>
                        <Input
                          type="number"
                          min={0}
                          className="bg-black/30 border-white/10"
                          value={config.ai.speech.tts.speaker_id}
                          onChange={(event) =>
                            updateConfig((draft) => {
                              draft.ai.speech.tts.speaker_id = Number(
                                event.target.value,
                              );
                            })
                          }
                        />
                      </div>
                      <div className="space-y-2">
                        <Label>{t("ai.models.tts_speed")}</Label>
                        <Input
                          type="number"
                          min={0.5}
                          max={2}
                          step={0.1}
                          className="bg-black/30 border-white/10"
                          value={config.ai.speech.tts.speed}
                          onChange={(event) =>
                            updateConfig((draft) => {
                              draft.ai.speech.tts.speed = Number(
                                event.target.value,
                              );
                            })
                          }
                        />
                      </div>
                    </div>
                    {runtimeReady === false ? (
                      <div className="rounded-md border border-amber-400/30 bg-amber-400/10 px-3 py-2 text-xs text-amber-100/90">
                        {t("ai.models.runtime_required")}
                      </div>
                    ) : null}
                    {sttReady === false ? (
                      <div className="rounded-md border border-amber-400/30 bg-amber-400/10 px-3 py-2 text-xs text-amber-100/90">
                        {t("ai.models.stt_required")}
                      </div>
                    ) : null}
                    {showTtsDownloadHint ? (
                      <div className="rounded-md border border-amber-400/30 bg-amber-400/10 px-3 py-2 text-xs text-amber-100/90">
                        {t("ai.models.tts_required")}
                      </div>
                    ) : null}
                  </CardContent>
                </Card>

                <Card className="border-white/10 bg-[#1A1D24] text-white">
                  <CardHeader>
                    <CardTitle className="text-[#FCE100]">
                      {t("ai.models.llm_title")}
                    </CardTitle>
                  </CardHeader>
                  <CardContent className="space-y-4">
                    <div className="space-y-3 rounded-lg border border-white/10 bg-black/20 p-3">
                      <div className="flex items-center justify-between gap-4">
                        <div>
                          <p className="text-sm text-white">{t("ai.models.llm_enabled")}</p>
                          <p className="text-xs text-white/45">
                            {t("ai.models.llm_enabled_hint")}
                          </p>
                        </div>
                        <Switch
                          checked={config.ai.llm.enabled}
                          onCheckedChange={(checked) =>
                            updateConfig((draft) => {
                              draft.ai.llm.enabled = checked;
                            })
                          }
                        />
                      </div>

                      <div className="flex items-center justify-between gap-4">
                        <div>
                          <p className="text-sm text-white">{t("ai.models.reply_enabled")}</p>
                          <p className="text-xs text-white/45">
                            {t("ai.models.reply_enabled_hint")}
                          </p>
                        </div>
                        <Switch
                          checked={config.ai.llm.reply_enabled}
                          onCheckedChange={(checked) =>
                            updateConfig((draft) => {
                              draft.ai.llm.reply_enabled = checked;
                            })
                          }
                        />
                      </div>

                      <div className="space-y-2">
                        <Label>{t("ai.models.context_event_count")}</Label>
                        <Select
                          value={String(config.ai.llm.context_event_count)}
                          onValueChange={(value) =>
                            updateConfig((draft) => {
                              draft.ai.llm.context_event_count = Number(value);
                            })
                          }
                        >
                          <SelectTrigger className="bg-black/30 text-white">
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent className="border-white/10 bg-[#1E2128] text-white">
                            {CONTEXT_EVENT_COUNT_OPTIONS.map((count) => (
                              <SelectItem key={count} value={String(count)}>
                                {count}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      </div>
                    </div>

                    <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
                      {renderLlmStageConfig(
                        "decision",
                        t("ai.models.decision_model_title"),
                      )}
                      {renderLlmStageConfig("reply", t("ai.models.reply_model_title"))}
                    </div>
                  </CardContent>
                </Card>

                {error ? (
                  <div className="rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-200">
                    {error}
                  </div>
                ) : null}
              </div>
            </TabsContent>

            <TabsContent value="debug" className="min-h-0 flex-1 overflow-y-auto p-4">
              <div className="space-y-4">
                <Card className="border-white/10 bg-[#1A1D24] text-white">
                  <CardHeader>
                    <CardTitle className="text-[#FCE100]">
                      {t("ai.debug.title")}
                    </CardTitle>
                  </CardHeader>
                  <CardContent className="space-y-4">
                    <div className="flex items-center justify-between gap-4 rounded-lg border border-white/10 bg-black/20 p-3">
                      <div>
                        <p className="text-sm text-white">{t("ai.debug.enabled")}</p>
                        <p className="text-xs text-white/45">
                          {t("ai.debug.enabled_hint")}
                        </p>
                      </div>
                      <Switch
                        checked={debugEnabled}
                        onCheckedChange={(checked) => void toggleDebugEnabled(checked)}
                      />
                    </div>

                    {debugEnabled ? (
                      <div className="space-y-4">
                        <Tabs
                          value={debugActiveTab}
                          onValueChange={(value) =>
                            setDebugActiveTab(value as AiDebugStage)
                          }
                          className="space-y-4"
                        >
                          <TabsList className="grid w-full grid-cols-3 bg-white/5">
                            <TabsTrigger value="stt">STT</TabsTrigger>
                            <TabsTrigger value="llm">LLM</TabsTrigger>
                            <TabsTrigger value="tts">TTS</TabsTrigger>
                          </TabsList>

                          <TabsContent value="stt" className="mt-0">
                            <div className="space-y-3 rounded-lg border border-white/10 bg-black/20 p-3">
                              <div>
                                <p className="text-sm font-medium text-white">
                                  {t("ai.debug.stt_title")}
                                </p>
                                <p className="mt-1 text-xs text-white/45">
                                  {t("ai.debug.stt_hint")}
                                </p>
                              </div>
                              <Button
                                className="bg-[#FCE100] text-black hover:bg-[#FCE100]/90"
                                disabled={
                                  Boolean(debugRunningStage) &&
                                  debugRunningStage !== "stt"
                                }
                                onClick={() => void toggleDebugSttTest()}
                              >
                                {debugRunningStage === "stt"
                                  ? t("ai.debug.stop_test")
                                  : t("ai.debug.start_test")}
                              </Button>
                            </div>
                          </TabsContent>

                          <TabsContent value="llm" className="mt-0">
                            <div className="space-y-3 rounded-lg border border-white/10 bg-black/20 p-3">
                              <div className="space-y-2">
                                <Label>{t("ai.debug.llm_input")}</Label>
                                <Textarea
                                  className="min-h-28 bg-black/30 border-white/10"
                                  value={llmDebugInput}
                                  onChange={(event) =>
                                    setLlmDebugInput(event.target.value)
                                  }
                                  placeholder={t("ai.debug.llm_placeholder")}
                                />
                              </div>
                              <Button
                                className="bg-[#FCE100] text-black hover:bg-[#FCE100]/90"
                                disabled={
                                  Boolean(debugRunningStage) &&
                                  debugRunningStage !== "llm"
                                }
                                onClick={() => void toggleDebugLlmTest()}
                              >
                                {debugRunningStage === "llm"
                                  ? t("ai.debug.stop_test")
                                  : t("ai.debug.start_test")}
                              </Button>
                            </div>
                          </TabsContent>

                          <TabsContent value="tts" className="mt-0">
                            <div className="space-y-3 rounded-lg border border-white/10 bg-black/20 p-3">
                              <div className="space-y-2">
                                <Label>{t("ai.debug.tts_input")}</Label>
                                <Textarea
                                  className="min-h-28 bg-black/30 border-white/10"
                                  value={ttsDebugInput}
                                  onChange={(event) =>
                                    setTtsDebugInput(event.target.value)
                                  }
                                  placeholder={t("ai.debug.tts_placeholder")}
                                />
                              </div>
                              <Button
                                className="bg-[#FCE100] text-black hover:bg-[#FCE100]/90"
                                disabled={
                                  Boolean(debugRunningStage) &&
                                  debugRunningStage !== "tts"
                                }
                                onClick={() => void toggleDebugTtsTest()}
                              >
                                {debugRunningStage === "tts"
                                  ? t("ai.debug.stop_test")
                                  : t("ai.debug.start_test")}
                              </Button>
                            </div>
                          </TabsContent>
                        </Tabs>

                        <div className="rounded-lg border border-white/10 bg-black/20">
                          <div className="flex items-center justify-between gap-3 border-b border-white/10 px-3 py-2">
                            <p className="text-sm font-medium text-white">
                              {t("ai.debug.logs")}
                            </p>
                            <Button
                              variant="outline"
                              className="h-8 border-white/10 bg-black/20 px-3 text-xs hover:bg-white/10"
                              onClick={() => setDebugLogs([])}
                            >
                              {t("ai.debug.clear_logs")}
                            </Button>
                          </div>
                          <div className="max-h-96 space-y-2 overflow-y-auto p-3">
                            {debugLogs.length === 0 ? (
                              <p className="text-sm text-white/45">
                                {t("ai.debug.empty_logs")}
                              </p>
                            ) : (
                              debugLogs.map((log) => {
                                const detail = formatDebugDetail(log.detail);

                                return (
                                  <div
                                    key={log.id}
                                    className={`rounded-lg border p-3 ${debugLevelClasses(
                                      log.level,
                                    )}`}
                                  >
                                    <div className="mb-2 flex flex-wrap items-center justify-between gap-2 text-xs">
                                      <div className="flex items-center gap-2 uppercase tracking-[0.14em]">
                                        <span>{log.stage}</span>
                                        <span>{log.level}</span>
                                        {log.elapsed_ms !== undefined ? (
                                          <span>{log.elapsed_ms}ms</span>
                                        ) : null}
                                      </div>
                                      <span className="text-white/35">
                                        {formatTimestamp(log.created_at_ms)}
                                      </span>
                                    </div>
                                    <p className="break-words text-sm">{log.message}</p>
                                    {detail ? (
                                      <pre className="mt-2 max-h-72 overflow-auto whitespace-pre-wrap break-all rounded-lg border border-white/10 bg-black/30 p-3 text-xs text-white/70">
                                        {detail}
                                      </pre>
                                    ) : null}
                                  </div>
                                );
                              })
                            )}
                          </div>
                        </div>
                      </div>
                    ) : null}
                  </CardContent>
                </Card>
              </div>
            </TabsContent>
      </Tabs>
    </div>
  );
}
