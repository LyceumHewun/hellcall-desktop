import { useState, useEffect } from "react";
import { KeySequence } from "./KeySequence";
import { mapWebEventToRustInput, KeyRecorder } from "./KeyRecorder";
import {
  Trash2,
  GripVertical,
  Keyboard,
  ChevronDown,
  ChevronUp,
  X,
} from "lucide-react";
import { Badge } from "./ui/badge";
import { CommandConfig } from "../../types/config";
import { useConfigStore } from "../../store/configStore";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useTranslation } from "react-i18next";

interface MacroCardProps {
  id: string;
  commandIndex: number;
  command: CommandConfig;
  onDelete?: () => void;
}

export function MacroCard({
  id,
  commandIndex,
  command,
  onDelete,
}: MacroCardProps) {
  const { t } = useTranslation();
  const { updateConfig } = useConfigStore();
  const [isExpanded, setIsExpanded] = useState(false);
  const [isRecording, setIsRecording] = useState(false);
  const [isRecordingShortcut, setIsRecordingShortcut] = useState(false);

  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
    zIndex: isDragging ? 50 : 1,
  };

  useEffect(() => {
    if (!isRecording) return;

    const handleInput = (e: KeyboardEvent | MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (e.type === "keydown") {
        const kbEvent = e as KeyboardEvent;
        if (kbEvent.key === "Escape" || kbEvent.key === "Enter") {
          setIsRecording(false);
          return;
        }

        if (kbEvent.key === "Backspace" || kbEvent.key === "Delete") {
          updateConfig((draft) => {
            draft.commands[commandIndex].keys.pop();
          });
          return;
        }
      }

      const physicalKey = mapWebEventToRustInput(e);
      const state = useConfigStore.getState();
      if (!state.config) return;

      const keyMap = state.config.key_map;
      let logicalKey: string | null = null;

      // Reverse lookup: Find the logical action corresponding to the pressed physical key
      for (const [logical, physical] of Object.entries(keyMap)) {
        if (JSON.stringify(physical) === JSON.stringify(physicalKey)) {
          logicalKey = logical;
          break;
        }
      }

      // Only push if a matching logical binding exists
      if (logicalKey) {
        const currentKeys = state.config.commands[commandIndex].keys;

        // 1. The THROW Lock Rule: If THROW is already in the array, reject all inputs
        if (currentKeys.includes("THROW")) {
          return;
        }

        // 2. The OPEN Rule: OPEN can only be the very first key
        if (logicalKey === "OPEN" && currentKeys.length > 0) {
          return;
        }

        // 3. Length Limit Rule: Reject if sequence is 11 or longer
        if (currentKeys.length >= 11) {
          return;
        }

        updateConfig((draft) => {
          draft.commands[commandIndex].keys.push(logicalKey as string);
        });
      }
    };

    const handleContextMenu = (e: MouseEvent) => e.preventDefault();

    window.addEventListener("keydown", handleInput as EventListener, {
      capture: true,
    });
    window.addEventListener("mousedown", handleInput as EventListener, {
      capture: true,
    });
    window.addEventListener("contextmenu", handleContextMenu, {
      capture: true,
    });

    return () => {
      window.removeEventListener("keydown", handleInput as EventListener, {
        capture: true,
      });
      window.removeEventListener("mousedown", handleInput as EventListener, {
        capture: true,
      });
      window.removeEventListener("contextmenu", handleContextMenu, {
        capture: true,
      });
    };
  }, [isRecording, commandIndex, updateConfig]);
  const removeAudio = (indexToRemove: number) => {
    updateConfig((draft) => {
      draft.commands[commandIndex].audio_files.splice(indexToRemove, 1);
    });
  };

  const handleAddAudio = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      const val = e.currentTarget.value.trim();
      if (val && !command.audio_files.includes(val)) {
        updateConfig((draft) => {
          draft.commands[commandIndex].audio_files.push(val);
        });
        e.currentTarget.value = "";
      }
    }
  };

  const isUnsaved = command.command.trim() === "" || command.keys.length === 0;

  return (
    <div
      ref={setNodeRef}
      style={style}
      className={`bg-[#1E2128] rounded border ${
        isRecording
          ? "border-[#D93A3A]"
          : isUnsaved
            ? "border-red-500/50 border-dashed"
            : "border-white/10"
      } overflow-hidden transition-colors relative`}
    >
      {/* Collapsed Row */}
      <div className="flex items-center gap-3 px-4 h-16 hover:bg-white/[0.02] transition-colors">
        {/* Drag Handle */}
        <button
          {...attributes}
          {...listeners}
          className="text-white/30 hover:text-white/60 transition-colors cursor-grab active:cursor-grabbing"
        >
          <GripVertical className="w-4 h-4" />
        </button>

        {/* Voice Trigger Input */}
        <input
          type="text"
          value={command.command}
          onChange={(e) =>
            updateConfig((draft) => {
              draft.commands[commandIndex].command = e.target.value;
            })
          }
          className="w-[20%] min-w-[140px] bg-transparent border-b border-white/10 px-2 py-1 text-white placeholder:text-white/30 focus:outline-none focus:border-[#FCE100]/60 transition-colors text-sm"
          placeholder={t("macros.card.trigger")}
        />

        {/* Macro Sequence */}
        <div className="flex-1 flex items-center">
          <KeySequence sequence={command.keys} compact />
          {isUnsaved && !isRecording && (
            <span className="text-[10px] text-red-400/80 uppercase tracking-wider ml-3">
              {t("macros.card.unsaved")}
            </span>
          )}
        </div>

        {/* Action Icons */}
        <div className="flex items-center gap-1">
          <button
            onClick={() => {
              if (!isRecording) {
                updateConfig((draft) => {
                  draft.commands[commandIndex].keys = [];
                });
              }
              setIsRecording(!isRecording);
            }}
            className={`p-2 rounded transition-colors ${
              isRecording
                ? "bg-[#D93A3A]/20 text-[#D93A3A] border border-[#D93A3A]"
                : "text-white/40 hover:text-[#FCE100] hover:bg-[#FCE100]/5"
            }`}
            title={
              isRecording
                ? t("macros.card.stop_recording")
                : t("macros.card.record_seq")
            }
          >
            <Keyboard className="w-4 h-4" />
          </button>

          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className="p-2 text-white/40 hover:text-[#FCE100] hover:bg-[#FCE100]/5 rounded transition-colors"
            title={t("macros.card.advanced")}
          >
            {isExpanded ? (
              <ChevronUp className="w-4 h-4" />
            ) : (
              <ChevronDown className="w-4 h-4" />
            )}
          </button>

          <button
            onClick={onDelete}
            className="p-2 text-white/40 hover:text-[#D93A3A] hover:bg-[#D93A3A]/5 rounded transition-colors"
            title={t("macros.card.delete")}
          >
            <Trash2 className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Expanded Panel */}
      {isExpanded && (
        <div className="bg-[#15171C] border-t border-white/10 border-l-2 border-l-[#FCE100] px-4 py-4">
          <div className="grid grid-cols-3 gap-4">
            {/* Engine Grammar */}
            <div className="col-span-2 flex flex-col gap-1.5">
              <label className="text-white/50 text-xs uppercase tracking-wide">
                {t("macros.card.grammar")}
              </label>
              <input
                type="text"
                value={command.grammar || ""}
                onChange={(e) =>
                  updateConfig((draft) => {
                    draft.commands[commandIndex].grammar = e.target.value;
                  })
                }
                className="bg-white/5 border border-white/10 rounded px-3 py-2 text-white text-sm placeholder:text-white/30 focus:outline-none focus:border-[#FCE100]/50 transition-colors font-mono"
              />
            </div>

            {/* Fallback Shortcut */}
            <div className="col-span-1 flex flex-col gap-1.5">
              <label className="text-white/50 text-xs uppercase tracking-wide">
                {t("macros.card.shortcut")}
              </label>
              <div className="flex items-center gap-2 h-[38px]">
                <KeyRecorder
                  actionName="Shortcut"
                  currentKeyCode={command.shortcut || "Unbound"}
                  isRecording={isRecordingShortcut}
                  onStartRecording={() => setIsRecordingShortcut(true)}
                  onKeyRecorded={(newKeyCode) => {
                    updateConfig((draft) => {
                      draft.commands[commandIndex].shortcut = newKeyCode as any;
                    });
                    setIsRecordingShortcut(false);
                  }}
                  onCancelRecording={() => setIsRecordingShortcut(false)}
                  onClear={() => {
                    updateConfig((draft) => {
                      draft.commands[commandIndex].shortcut = null;
                    });
                  }}
                />
              </div>
            </div>

            {/* Response Audio */}
            <div className="col-span-3 flex flex-col gap-1.5">
              <label className="text-white/50 text-xs uppercase tracking-wide">
                {t("macros.card.audio")}
              </label>
              <div className="flex flex-wrap items-center gap-2 bg-white/5 border border-white/10 rounded px-3 py-2 min-h-[38px] focus-within:border-[#FCE100]/50 transition-colors">
                {command.audio_files.map((audio, i) => (
                  <Badge
                    key={i}
                    variant="secondary"
                    className="bg-zinc-800 text-zinc-300 hover:bg-zinc-700 flex items-center"
                  >
                    {audio}
                    <button
                      type="button"
                      className="ml-0.5 text-zinc-400 hover:text-white rounded-full p-0.5 hover:bg-white/10 transition-colors focus:outline-none"
                      onClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        removeAudio(i);
                      }}
                    >
                      <X className="w-3 h-3" />
                    </button>
                  </Badge>
                ))}
                <input
                  type="text"
                  onKeyDown={handleAddAudio}
                  className="flex-1 bg-transparent text-white text-sm placeholder:text-white/30 focus:outline-none min-w-[120px]"
                  placeholder={t("macros.card.audio_placeholder")}
                />
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
