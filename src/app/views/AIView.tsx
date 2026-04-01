import { useEffect, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Bot, Plus, Trash2, Waves } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useConfigStore } from "../../store/configStore";
import { useAiStore } from "../../store/aiStore";
import { useEngineStore } from "../../store/engineStore";
import { Button } from "../components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { Input } from "../components/ui/input";
import { Label } from "../components/ui/label";
import { ScrollArea } from "../components/ui/scroll-area";
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

function formatTimestamp(value: number) {
  return new Date(value).toLocaleString();
}

export function AIView() {
  const { t } = useTranslation();
  const { config, updateConfig } = useConfigStore();
  const selectedDevice = useEngineStore((state) => state.selectedDevice);
  const {
    sessions,
    currentSessionId,
    currentSession,
    isRecording,
    isStreaming,
    streamingText,
    lastTranscript,
    isLoadingSessions,
    isLoadingSession,
    error,
    setError,
    clearError,
    setRecording,
    setStreaming,
    appendStreamingText,
    resetStreamingText,
    setLastTranscript,
    fetchSessions,
    selectSession,
    createSession,
    deleteSession,
  } = useAiStore();
  const isManualHoldRef = useRef(false);

  useEffect(() => {
    fetchSessions();
  }, [fetchSessions]);

  useEffect(() => {
    if (!config) {
      return;
    }

    invoke("sync_ai_runtime", {
      config,
      deviceName: selectedDevice,
      sessionId: currentSessionId,
    }).catch((syncError) => {
      const message =
        syncError instanceof Error
          ? syncError.message
          : String(syncError ?? t("ai.errors.sync_runtime"));
      console.error("Failed to sync AI runtime:", syncError);
      setError(message);
    });
  }, [config, currentSessionId, selectedDevice, setError, t]);

  useEffect(() => {
    let mounted = true;
    let unlistenRecording: UnlistenFn | null = null;
    let unlistenTranscript: UnlistenFn | null = null;
    let unlistenError: UnlistenFn | null = null;
    let unlistenChatState: UnlistenFn | null = null;
    let unlistenChatDelta: UnlistenFn | null = null;
    let unlistenChatFinished: UnlistenFn | null = null;

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
      audio_path: string;
    }>("ai-transcription-ready", async (event) => {
      if (!mounted) {
        return;
      }

      setLastTranscript(event.payload.transcript);
      clearError();
      await fetchSessions();
      await selectSession(event.payload.session_id);
      resetStreamingText();
      await invoke("start_ai_chat_stream", {
        sessionId: event.payload.session_id,
      }).catch((chatError) => {
        const message =
          chatError instanceof Error
            ? chatError.message
            : String(chatError ?? t("ai.errors.start_chat"));
        setError(message);
        toast.error(message);
      });
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
        fetchSessions().catch(console.error);
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
      async (event) => {
        if (!mounted) {
          return;
        }
        await fetchSessions();
        await selectSession(event.payload.session_id);
      },
    ).then((fn) => {
      unlistenChatFinished = fn;
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
    };
  }, [
    appendStreamingText,
    clearError,
    currentSessionId,
    fetchSessions,
    resetStreamingText,
    selectSession,
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
        system_prompt: t("ai.agent.new_prompt"),
        chat_model: draft.ai.default_chat_model,
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
    <>
      <div className="border-b border-white/10 bg-gradient-to-b from-[#0F1115] to-transparent p-6 backdrop-blur-sm">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <h1
              style={{ fontFamily: "var(--font-family-tech)" }}
              className="mb-1 tracking-wider text-white uppercase"
            >
              {t("ai.title")}
            </h1>
            <p className="max-w-xl text-sm text-white/50">{t("ai.subtitle")}</p>
          </div>
          <Button
            variant="outline"
            className="shrink-0 cursor-pointer border-[#FCE100]/40 bg-[#FCE100]/10 text-[#FCE100] hover:bg-[#FCE100]/20"
            onClick={async () => {
              await createSession();
              const nextError = useAiStore.getState().error;
              if (nextError) {
                toast.error(nextError);
              }
            }}
          >
            <Plus className="mr-2 h-4 w-4" />
            {t("ai.new_session")}
          </Button>
        </div>
      </div>

      <div className="flex flex-1 overflow-hidden">
        <aside className="w-72 shrink-0 border-r border-white/10 bg-[#101318] p-4">
          <div className="mb-4 flex items-center gap-3 rounded-lg border border-[#FCE100]/20 bg-[#FCE100]/10 p-3">
            <Bot className="h-5 w-5 text-[#FCE100]" />
            <div className="min-w-0">
              <p className="text-sm font-medium text-white">{t("ai.mode_active")}</p>
              <p className="text-xs text-white/50">{t("ai.mode_active_hint")}</p>
            </div>
          </div>

          <div className="space-y-2">
            <p className="text-xs uppercase tracking-[0.2em] text-white/40">
              {t("ai.session_list")}
            </p>
            <ScrollArea className="h-[calc(100vh-250px)]">
              <div className="space-y-2 pr-3">
                {isLoadingSessions ? (
                  <div className="rounded-lg border border-white/10 bg-black/20 p-3 text-sm text-white/50">
                    {t("ai.loading_sessions")}
                  </div>
                ) : sessions.length === 0 ? (
                  <div className="rounded-lg border border-dashed border-white/10 bg-black/20 p-4 text-sm text-white/50">
                    {t("ai.empty_sessions")}
                  </div>
                ) : (
                  sessions.map((session) => (
                    <button
                      key={session.id}
                      onClick={() => selectSession(session.id)}
                      className={`w-full rounded-lg border p-3 text-left transition-colors ${
                        currentSessionId === session.id
                          ? "border-[#FCE100]/50 bg-[#FCE100]/10"
                          : "border-white/10 bg-black/20 hover:bg-white/5"
                      }`}
                    >
                      <div className="flex items-start justify-between gap-3">
                        <div className="min-w-0">
                          <p className="truncate text-sm font-medium text-white">
                            {session.title}
                          </p>
                          <p className="mt-1 text-xs text-white/45">
                            {formatTimestamp(session.updated_at_ms)}
                          </p>
                        </div>
                        <span className="shrink-0 rounded-full bg-white/10 px-2 py-0.5 text-[10px] text-white/60">
                          {session.message_count}
                        </span>
                      </div>
                    </button>
                  ))
                )}
              </div>
            </ScrollArea>
          </div>
        </aside>

        <main className="flex min-w-0 flex-1 flex-col bg-[#0C0E12]">
          <Tabs defaultValue="conversation" className="flex min-h-0 flex-1 flex-col">
            <div className="border-b border-white/10 px-4 py-3">
              <TabsList className="grid w-full grid-cols-4 bg-white/5">
                <TabsTrigger value="conversation">{t("ai.tabs.conversation")}</TabsTrigger>
                <TabsTrigger value="agent">{t("ai.tabs.agent")}</TabsTrigger>
                <TabsTrigger value="skills">{t("ai.tabs.skills")}</TabsTrigger>
                <TabsTrigger value="models">{t("ai.tabs.models")}</TabsTrigger>
              </TabsList>
            </div>

            <TabsContent value="conversation" className="min-h-0 flex-1">
              <div className="flex h-full flex-col">
                <div className="flex-1 overflow-y-auto p-4">
                  {error ? (
                    <div className="mb-4 rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">
                      {error}
                    </div>
                  ) : null}
                  {currentSession ? (
                    <div className="rounded-2xl border border-white/10 bg-[#171A20] p-4">
                      <div className="mb-4 flex items-start justify-between gap-3">
                        <div className="min-w-0">
                          <h2 className="truncate text-lg font-semibold text-white">
                            {currentSession.title}
                          </h2>
                          <p className="text-xs text-white/45">
                            {t("ai.session_meta", {
                              created: formatTimestamp(currentSession.created_at_ms),
                              updated: formatTimestamp(currentSession.updated_at_ms),
                            })}
                          </p>
                        </div>
                          <Button
                            variant="ghost"
                            className="shrink-0 cursor-pointer text-white/50 hover:bg-red-500/10 hover:text-red-300"
                            onClick={async () => {
                              await deleteSession(currentSession.id);
                              const nextError = useAiStore.getState().error;
                              if (nextError) {
                                toast.error(nextError);
                              }
                            }}
                          >
                          <Trash2 className="h-4 w-4" />
                        </Button>
                      </div>

                      {isLoadingSession ? (
                        <p className="text-sm text-white/50">{t("ai.loading_session")}</p>
                      ) : currentSession.events.length === 0 ? (
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
                          {currentSession.events.map((event) => (
                            <div
                              key={event.id}
                              className="rounded-xl border border-white/10 bg-black/20 p-4"
                            >
                              <div className="mb-2 flex items-center justify-between gap-3 text-xs text-white/40">
                                <span className="truncate">{event.kind}</span>
                                <span className="shrink-0">
                                  {formatTimestamp(event.created_at_ms)}
                                </span>
                              </div>
                              <p className="whitespace-pre-wrap break-words text-sm text-white/75">
                                {event.text || t("ai.empty_event")}
                              </p>
                            </div>
                          ))}
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
                    </div>
                  ) : (
                    <div className="rounded-2xl border border-dashed border-white/10 bg-[#171A20] p-8 text-center">
                      <p className="text-sm text-white/60">{t("ai.select_or_create")}</p>
                    </div>
                  )}
                </div>

                <div className="border-t border-white/10 bg-[#12151A] p-4">
                  <div className="flex flex-col gap-3 rounded-2xl border border-[#FCE100]/20 bg-[#FCE100]/5 px-4 py-3">
                    <div>
                      <p className="text-sm font-medium text-white">{t("ai.ptt_title")}</p>
                      <p className="text-xs text-white/45">{t("ai.ptt_body")}</p>
                    </div>
                    {lastTranscript ? (
                      <div className="rounded-lg border border-white/10 bg-black/20 px-3 py-2">
                        <p className="mb-1 text-[10px] uppercase tracking-[0.2em] text-white/35">
                          {t("ai.latest_transcript")}
                        </p>
                        <p className="text-sm text-white/75">{lastTranscript}</p>
                      </div>
                    ) : null}
                    <Button
                      className={`w-full text-black ${
                        isRecording
                          ? "bg-[#FCE100] hover:bg-[#FCE100]/90"
                          : "bg-[#FCE100]/85 hover:bg-[#FCE100]"
                      }`}
                      onMouseDown={() => void startManualRecording()}
                      onMouseUp={() => void stopManualRecording()}
                      onMouseLeave={() => void stopManualRecording()}
                      onTouchStart={() => void startManualRecording()}
                      onTouchEnd={() => void stopManualRecording()}
                    >
                      {isRecording ? t("ai.ptt_button_recording") : t("ai.ptt_button")}
                    </Button>
                  </div>
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
                    <CardTitle className="text-[#FCE100]">{t("ai.models.title")}</CardTitle>
                  </CardHeader>
                  <CardContent className="space-y-4">
                    <div className="space-y-2">
                      <Label>{t("ai.models.base_url")}</Label>
                      <Input
                        className="bg-black/30 border-white/10"
                        value={config.ai.base_url}
                        onChange={(event) =>
                          updateConfig((draft) => {
                            draft.ai.base_url = event.target.value;
                          })
                        }
                      />
                    </div>
                    <div className="space-y-2">
                      <Label>{t("ai.models.api_key")}</Label>
                      <Input
                        type="password"
                        className="bg-black/30 border-white/10"
                        value={config.ai.api_key}
                        onChange={(event) =>
                          updateConfig((draft) => {
                            draft.ai.api_key = event.target.value;
                          })
                        }
                      />
                    </div>
                    <div className="space-y-2">
                      <Label>{t("ai.models.chat_model")}</Label>
                      <Input
                        className="bg-black/30 border-white/10"
                        value={config.ai.default_chat_model}
                        onChange={(event) =>
                          updateConfig((draft) => {
                            draft.ai.default_chat_model = event.target.value;
                          })
                        }
                      />
                    </div>
                    <div className="space-y-2">
                      <Label>{t("ai.models.asr_model")}</Label>
                      <Input
                        className="bg-black/30 border-white/10"
                        value={config.ai.default_asr_model}
                        onChange={(event) =>
                          updateConfig((draft) => {
                            draft.ai.default_asr_model = event.target.value;
                          })
                        }
                      />
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
          </Tabs>
        </main>
      </div>
    </>
  );
}
