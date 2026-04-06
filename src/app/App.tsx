import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
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
import { Toaster } from "sonner";
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
  const currentSessionId = useAiStore((state) => state.currentSessionId);
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
