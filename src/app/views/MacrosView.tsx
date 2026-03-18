import { Plus } from "lucide-react";
import { MacroCard } from "../components/MacroCard";
import { useConfigStore } from "../../store/configStore";
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  DragEndEvent,
} from "@dnd-kit/core";
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";

export function MacrosView() {
  const { config, updateConfig } = useConfigStore();

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 5,
      },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );

  const addMacro = () => {
    updateConfig((draft) => {
      draft.commands.push({
        _frontendId: crypto.randomUUID(),
        command: "",
        grammar: null,
        shortcut: null,
        keys: [],
        audio_files: [],
      });
    });
  };

  const deleteMacro = (index: number) => {
    updateConfig((draft) => {
      draft.commands.splice(index, 1);
    });
  };

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;

    if (over && active.id !== over.id) {
      updateConfig((draft) => {
        const oldIndex = draft.commands.findIndex(
          (cmd) => cmd._frontendId === active.id,
        );
        const newIndex = draft.commands.findIndex(
          (cmd) => cmd._frontendId === over.id,
        );

        if (oldIndex !== -1 && newIndex !== -1) {
          draft.commands = arrayMove(draft.commands, oldIndex, newIndex);
        }
      });
    }
  };

  if (!config) return null;

  return (
    <>
      <div className="border-b border-white/10 p-6 shrink-0 bg-gradient-to-b from-[#0F1115] to-transparent backdrop-blur-sm">
        <div className="flex items-center justify-between">
          <div>
            <h1
              style={{ fontFamily: "var(--font-family-tech)" }}
              className="tracking-wider text-white mb-1"
            >
              VOICE COMMAND ARSENAL
            </h1>
            <p className="text-white/50 text-sm">
              Configure tactical voice-activated macros
            </p>
          </div>
          <button
            onClick={addMacro}
            className="flex items-center gap-2 px-4 py-2.5 border-2 border-[#FCE100] text-[#FCE100] rounded hover:bg-[#FCE100]/10 transition-colors"
          >
            <Plus className="w-4 h-4" />
            <span>Add New Macro</span>
          </button>
        </div>
      </div>
      <div className="flex-1 overflow-y-auto p-6">
        <div className="max-w-6xl mx-auto flex flex-col gap-2">
          <DndContext
            sensors={sensors}
            collisionDetection={closestCenter}
            onDragEnd={handleDragEnd}
          >
            <SortableContext
              items={config.commands.map((cmd) => cmd._frontendId!)}
              strategy={verticalListSortingStrategy}
            >
              {config.commands.map((cmd, index) => {
                const id = cmd._frontendId!;
                return (
                  <MacroCard
                    key={id}
                    id={id}
                    commandIndex={index}
                    command={cmd}
                    onDelete={() => deleteMacro(index)}
                  />
                );
              })}
            </SortableContext>
          </DndContext>
        </div>
      </div>
    </>
  );
}
