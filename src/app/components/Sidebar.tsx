import { useEffect } from "react";
import {
  Settings,
  Keyboard,
  Command,
  Terminal,
  SatelliteDish,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { useConfigStore } from "../../store/configStore";
import { useEngineStore } from "../../store/engineStore";
import {
  buildEngineStartSnapshot,
  createEngineStartSignature,
} from "../../store/engineConfig";
import { useTranslation } from "react-i18next";

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
    lastStartedConfigSignature,
    setLastStartedConfigSignature,
    selectedDevice,
    selectedVoskModelId,
    selectedVoskModelReady,
    setSelectedVoskModelReady,
    selectedVisionModelId,
    setSelectedVisionModelReady,
  } = useEngineStore();

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

  const isActive = status === "ACTIVE";
  const isStarting = status === "STARTING";
  const isStartDisabled =
    isStarting || (!isActive && selectedVoskModelReady === false);
  const showRestartReminder =
    isActive &&
    currentEngineConfigSignature !== null &&
    lastStartedConfigSignature !== null &&
    currentEngineConfigSignature !== lastStartedConfigSignature;

  return (
    <div className="w-64 bg-[#0F1115] border-r border-white/10 flex flex-col p-4 gap-6">
      {/* Status Toggle */}
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

        {selectedVoskModelReady === false && !isActive ? (
          <p className="px-1 text-xs text-amber-300/80">
            {t("settings.model_required")}
          </p>
        ) : null}
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
