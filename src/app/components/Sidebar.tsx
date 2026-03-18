import { useState } from "react";
import { Settings, Keyboard, Command, Terminal } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { useConfigStore } from "../../store/configStore";
import { AppConfig } from "../../types/config";
import { useTranslation } from "react-i18next";

interface SidebarProps {
  activeNav: string;
  setActiveNav: (nav: string) => void;
}

type EngineStatus = "OFFLINE" | "STARTING" | "ACTIVE";

export function Sidebar({ activeNav, setActiveNav }: SidebarProps) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<EngineStatus>("OFFLINE");

  const toggleEngine = async () => {
    if (status === "OFFLINE") {
      setStatus("STARTING");
      try {
        const state = useConfigStore.getState();
        if (!state.config) throw new Error("Config not loaded");

        const sanitizedConfig = JSON.parse(
          JSON.stringify(state.config),
        ) as AppConfig;
        sanitizedConfig.commands = sanitizedConfig.commands
          .filter((cmd) => cmd.command.trim() !== "" && cmd.keys.length > 0)
          .map((cmd: any) => {
            if (cmd.grammar && cmd.grammar.trim() === "") {
              cmd.grammar = null;
            }
            delete cmd._frontendId;
            return cmd;
          });

        await invoke("start_engine", { config: sanitizedConfig });
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

  const isActive = status === "ACTIVE";
  const isStarting = status === "STARTING";

  return (
    <div className="w-64 bg-[#0F1115] border-r border-white/10 flex flex-col p-4 gap-6">
      {/* Status Toggle */}
      <button
        onClick={toggleEngine}
        disabled={isStarting}
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

      {/* Navigation */}
      <nav className="flex flex-col gap-1">
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

      <style>{`
        @keyframes pulse {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.6; }
        }
      `}</style>
    </div>
  );
}
