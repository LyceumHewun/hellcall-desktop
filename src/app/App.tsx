import { useState } from "react";
import { CustomTitlebar } from "./components/CustomTitlebar";
import { Sidebar } from "./components/Sidebar";
import { MacroCard } from "./components/MacroCard";
import { Plus } from "lucide-react";
import { GlobalSettingsView } from "./views/GlobalSettingsView";
import { KeyBindingsView } from "./views/KeyBindingsView";

export default function App() {
  const [activeNav, setActiveNav] = useState("macros");
  const [macros, setMacros] = useState([
    {
      id: "1",
      voiceTrigger: "orbital strike",
      engineGrammar: "[orbital] [strike|bombardment]",
      responseAudio: ["confirm_orbital.wav"],
    },
    {
      id: "2",
      voiceTrigger: "resupply",
      engineGrammar: "[resupply|ammo|supplies]",
      responseAudio: ["confirm_resupply.wav"],
    },
    {
      id: "3",
      voiceTrigger: "eagle airstrike",
      engineGrammar: "[eagle] [airstrike|air strike]",
      responseAudio: ["confirm_eagle.wav"],
    },
    {
      id: "4",
      voiceTrigger: "reinforce",
      engineGrammar: "[reinforce|reinforcement|backup]",
      responseAudio: ["confirm_reinforce.wav"],
    },
    {
      id: "5",
      voiceTrigger: "sos beacon",
      engineGrammar: "[sos|emergency] [beacon]",
      responseAudio: ["confirm_sos.wav"],
    },
  ]);

  const addMacro = () => {
    const newMacro = {
      id: Date.now().toString(),
      voiceTrigger: "",
      engineGrammar: "",
      responseAudio: [],
    };
    setMacros([...macros, newMacro]);
  };

  const deleteMacro = (id: string) => {
    setMacros(macros.filter((m) => m.id !== id));
  };

  return (
    <div className="h-screen w-screen flex flex-col overflow-hidden rounded-lg border border-zinc-800 bg-[#0F1115]">
      <CustomTitlebar />

      <div className="flex-1 flex overflow-hidden">
        <Sidebar activeNav={activeNav} setActiveNav={setActiveNav} />

        {/* Main Content Area */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Views */}
          {activeNav === "macros" && (
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
                  {macros.map((macro) => (
                    <MacroCard
                      key={macro.id}
                      initialData={macro as any}
                      onDelete={() => deleteMacro(macro.id)}
                    />
                  ))}
                </div>
              </div>
            </>
          )}
          {activeNav === "settings" && <GlobalSettingsView />}
          {activeNav === "keybindings" && <KeyBindingsView />}
        </div>
      </div>
    </div>
  );
}
