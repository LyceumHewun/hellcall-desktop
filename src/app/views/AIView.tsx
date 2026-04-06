import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Plus, Trash2, Waves } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useConfigStore } from "../../store/configStore";
import { useAiStore } from "../../store/aiStore";
import { useEngineStore } from "../../store/engineStore";
import { AiLiveToolActivity, AiSessionEvent } from "../../types/ai";
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
    currentSessionId,
    currentSession,
    liveToolActivities,
    isRecording,
    isStreaming,
    streamingText,
    isLoadingSession,
    error,
    setError,
    clearError,
    setRecording,
    setStreaming,
    appendStreamingText,
    resetStreamingText,
    pushLiveToolActivity,
    resetLiveToolActivities,
    setLastTranscript,
    fetchSession,
  } = useAiStore();
  const isManualHoldRef = useRef(false);

  useEffect(() => {
    fetchSession();
  }, [fetchSession]);

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

  useEffect(() => {
    let mounted = true;
    let unlistenRecording: UnlistenFn | null = null;
    let unlistenTranscript: UnlistenFn | null = null;
    let unlistenError: UnlistenFn | null = null;
    let unlistenChatState: UnlistenFn | null = null;
    let unlistenChatDelta: UnlistenFn | null = null;
    let unlistenChatFinished: UnlistenFn | null = null;
    let unlistenToolEvent: UnlistenFn | null = null;

    const recordingPromise = listen<{ recording: boolean }>(
      "ai-recording-state",
      (event) => {
        if (!mounted) {
          return;
        }
        setRecording(event.payload.recording);
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
      await fetchSession();
      resetStreamingText();
      resetLiveToolActivities();
    }).then((fn) => {
      unlistenTranscript = fn;
      return fn;
    });

    const errorPromise = listen<{ message: string }>("ai-recording-error", (event) => {
      if (!mounted) {
        return;
      }
      console.error("AI recording error:", event.payload.message);
      setRecording(false);
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
      if (!event.payload.streaming) {
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
      },
    ).then((fn) => {
      unlistenChatDelta = fn;
      return fn;
    });

    const chatFinishedPromise = listen<{ session_id: string; message: string }>(
      "ai-chat-finished",
      async () => {
        if (!mounted) {
          return;
        }
        await fetchSession();
      },
    ).then((fn) => {
      unlistenChatFinished = fn;
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
    setRecording,
    setStreaming,
    t,
  ]);

  const startManualRecording = async () => {
    if (isManualHoldRef.current || isRecording) {
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
      setRecording(false);
      setError(message);
      toast.error(message);
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

  const currentProvider = useMemo(() => {
    if (!config) {
      return null;
    }

    return (
      config.ai.llm.providers.find(
        (provider) => provider.id === config.ai.llm.selected_provider_id,
      ) ??
      config.ai.llm.providers[0] ??
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
  const pttButtonLabel = isRecording
    ? t("ai.ptt_button_recording")
    : aiStatus === "WARMING_UP"
      ? t("ai.ptt_button_warming_up")
      : aiStatus === "READY"
        ? t("ai.ptt_button")
        : t("ai.ptt_button_disabled");
  const pttDisabled = requiresSpeechDownload || !isAiReady;

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

  const updateCurrentProvider = (
    updater: (draft: NonNullable<typeof currentProvider>) => void,
  ) => {
    if (!currentProvider) {
      return;
    }

    updateConfig((draft) => {
      const target = draft.ai.llm.providers.find(
        (provider) => provider.id === currentProvider.id,
      );
      if (!target) {
        return;
      }
      updater(target as NonNullable<typeof currentProvider>);
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
        system_prompt: t("ai.agent.new_prompt"),
        chat_model: "",
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

  const addProvider = () => {
    if (!config) {
      return;
    }

    const nextId = crypto.randomUUID();
    updateConfig((draft) => {
      draft.ai.llm.providers.push({
        id: nextId,
        name: "Custom Provider",
        kind: "openai_compatible",
        base_url: "https://api.openai.com/v1",
        api_key: "",
        chat_model: "",
        is_builtin: false,
      });
      draft.ai.llm.selected_provider_id = nextId;
    });
  };

  const deleteCurrentProvider = () => {
    if (
      !config ||
      !currentProvider ||
      currentProvider.is_builtin ||
      config.ai.llm.providers.length <= 1
    ) {
      return;
    }

    updateConfig((draft) => {
      draft.ai.llm.providers = draft.ai.llm.providers.filter(
        (provider) => provider.id !== currentProvider.id,
      );
      draft.ai.llm.selected_provider_id = draft.ai.llm.providers[0]?.id ?? "";
    });
  };

  if (!config) {
    return null;
  }

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
          <TabsList className="grid w-full grid-cols-4 bg-white/5">
            <TabsTrigger value="conversation">{t("ai.tabs.conversation")}</TabsTrigger>
            <TabsTrigger value="agent">{t("ai.tabs.agent")}</TabsTrigger>
            <TabsTrigger value="skills">{t("ai.tabs.skills")}</TabsTrigger>
            <TabsTrigger value="models">{t("ai.tabs.models")}</TabsTrigger>
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
                            <div className="space-y-3">
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

                          {isStreaming && streamingText ? (
                            <div className="rounded-xl border border-[#FCE100]/30 bg-[#FCE100]/10 p-4">
                              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/45">
                                <span>{t("ai.streaming_label")}</span>
                                <span>{t("ai.streaming_status")}</span>
                              </div>
                              <p className="whitespace-pre-wrap break-words text-sm text-white/85">
                                {streamingText}
                              </p>
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
                onMouseDown={() => void startManualRecording()}
                onMouseUp={() => void stopManualRecording()}
                onMouseLeave={() => void stopManualRecording()}
                onTouchStart={() => void startManualRecording()}
                onTouchEnd={() => void stopManualRecording()}
              >
                {pttButtonLabel}
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
                        <Label>{t("ai.agent.system_prompt")}</Label>
                        <Textarea
                          className="min-h-40 bg-black/30 border-white/10"
                          value={currentAgent.system_prompt}
                          onChange={(event) =>
                            updateCurrentAgent((draft) => {
                              draft.system_prompt = event.target.value;
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
                    <div className="flex gap-2">
                      <Select
                        value={config.ai.llm.selected_provider_id}
                        onValueChange={(value) =>
                          updateConfig((draft) => {
                            draft.ai.llm.selected_provider_id = value;
                          })
                        }
                      >
                        <SelectTrigger className="min-w-0 flex-1 bg-black/30 text-white">
                          <SelectValue placeholder={t("ai.models.provider_select")} />
                        </SelectTrigger>
                        <SelectContent className="border-white/10 bg-[#1E2128] text-white">
                          {config.ai.llm.providers.map((provider) => (
                            <SelectItem key={provider.id} value={provider.id}>
                              {provider.name}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>

                      <Button
                        variant="outline"
                        className="cursor-pointer border-white/10 bg-black/20 hover:bg-white/10"
                        onClick={addProvider}
                      >
                        <Plus className="h-4 w-4" />
                      </Button>
                      <Button
                        variant="outline"
                        className="cursor-pointer border-white/10 bg-black/20 hover:bg-red-500/10 hover:text-red-300"
                        disabled={
                          !currentProvider ||
                          currentProvider.is_builtin ||
                          config.ai.llm.providers.length <= 1
                        }
                        onClick={deleteCurrentProvider}
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </div>

                    {currentProvider ? (
                      <div className="space-y-4">
                        <div className="space-y-2">
                          <Label>{t("ai.models.provider_name")}</Label>
                          <Input
                            className="bg-black/30 border-white/10"
                            value={currentProvider.name}
                            onChange={(event) =>
                              updateCurrentProvider((draft) => {
                                draft.name = event.target.value;
                              })
                            }
                          />
                        </div>
                        <div className="space-y-2">
                          <Label>{t("ai.models.provider_kind")}</Label>
                          <Select
                            value={currentProvider.kind}
                            onValueChange={(value: "siliconflow" | "openai_compatible") =>
                              updateCurrentProvider((draft) => {
                                draft.kind = value;
                              })
                            }
                          >
                            <SelectTrigger className="bg-black/30 text-white">
                              <SelectValue />
                            </SelectTrigger>
                            <SelectContent className="border-white/10 bg-[#1E2128] text-white">
                              <SelectItem value="siliconflow">SiliconFlow</SelectItem>
                              <SelectItem value="openai_compatible">
                                OpenAI Compatible
                              </SelectItem>
                            </SelectContent>
                          </Select>
                        </div>
                        <div className="space-y-2">
                          <Label>{t("ai.models.base_url")}</Label>
                          <Input
                            className="bg-black/30 border-white/10"
                            value={currentProvider.base_url}
                            onChange={(event) =>
                              updateCurrentProvider((draft) => {
                                draft.base_url = event.target.value;
                              })
                            }
                          />
                        </div>
                        <div className="space-y-2">
                          <Label>{t("ai.models.api_key")}</Label>
                          <Input
                            type="password"
                            className="bg-black/30 border-white/10"
                            value={currentProvider.api_key}
                            onChange={(event) =>
                              updateCurrentProvider((draft) => {
                                draft.api_key = event.target.value;
                              })
                            }
                          />
                        </div>
                        <div className="space-y-2">
                          <Label>{t("ai.models.chat_model")}</Label>
                          <Input
                            className="bg-black/30 border-white/10"
                            value={currentProvider.chat_model}
                            onChange={(event) =>
                              updateCurrentProvider((draft) => {
                                draft.chat_model = event.target.value;
                              })
                            }
                          />
                        </div>
                      </div>
                    ) : null}
                  </CardContent>
                </Card>

                {error ? (
                  <div className="rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-sm text-red-200">
                    {error}
                  </div>
                ) : null}
              </div>
            </TabsContent>
      </Tabs>
    </div>
  );
}
