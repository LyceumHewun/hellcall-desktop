import { useState, useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { exit } from "@tauri-apps/plugin-process";
import { CustomTitlebar } from "./components/CustomTitlebar";
import { Sidebar } from "./components/Sidebar";
import { UpdaterDialog } from "./components/UpdaterDialog";
import { Loader2 } from "lucide-react";
import { GlobalSettingsView } from "./views/GlobalSettingsView";
import { KeyBindingsView } from "./views/KeyBindingsView";
import { MacrosView } from "./views/MacrosView";
import { LogView } from "./views/LogView";
import { StratagemsView } from "./views/StratagemsView";
import { Toaster } from "sonner";
import { toast } from "sonner";
import { useConfigStore } from "../store/configStore";
import { CloseBehavior } from "../types/config";
import { CloseBehaviorDialog } from "./components/CloseBehaviorDialog";
import { useTranslation } from "react-i18next";

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
  const { config, isLoading, fetchConfig, updateConfigImmediately } =
    useConfigStore();
  const { t, i18n } = useTranslation();
  const [activeNav, setActiveNav] = useState("macros");
  const [isCloseDialogOpen, setIsCloseDialogOpen] = useState(false);

  useEffect(() => {
    fetchConfig();
  }, [fetchConfig]);

  useEffect(() => {
    if (!isLoading && config) {
      getCurrentWindow().show();
    }
  }, [isLoading, config]);

  useEffect(() => {
    const updateTrayLanguage = (language: string) => {
      invoke("set_tray_language", { language }).catch(console.error);
    };
    updateTrayLanguage(i18n.language);
    i18n.on("languageChanged", updateTrayLanguage);
    return () => i18n.off("languageChanged", updateTrayLanguage);
  }, [i18n]);

  const applyCloseBehavior = async (
    behavior: Exclude<CloseBehavior, "ask">,
  ) => {
    if (behavior === "minimize_to_tray") {
      await getCurrentWindow().hide();
    } else {
      await exit(0);
    }
  };

  const handleWindowClose = () => {
    if (!config) return;
    if (config.close_behavior === "ask") {
      setIsCloseDialogOpen(true);
      return;
    }
    applyCloseBehavior(config.close_behavior).catch((error) => {
      console.error(error);
      toast.error(t("close_dialog.close_failed"));
    });
  };

  const handleCloseConfirm = async (
    behavior: Exclude<CloseBehavior, "ask">,
    remember: boolean,
  ) => {
    if (remember) {
      try {
        await updateConfigImmediately((draft) => {
          draft.close_behavior = behavior;
        });
      } catch (error) {
        console.error(error);
        toast.error(t("close_dialog.save_failed"));
        return;
      }
    }

    try {
      setIsCloseDialogOpen(false);
      await applyCloseBehavior(behavior);
    } catch (error) {
      console.error(error);
      setIsCloseDialogOpen(true);
      toast.error(t("close_dialog.close_failed"));
    }
  };

  if (isLoading || !config) {
    return (
      <div className="h-screen w-screen flex items-center justify-center bg-[#0F1115]">
        <Loader2 className="w-8 h-8 text-[#FCE100] animate-spin" />
      </div>
    );
  }

  return (
    <div className="h-screen w-screen flex flex-col overflow-hidden rounded-lg border border-zinc-800 bg-[#0F1115]">
      <CustomTitlebar onClose={handleWindowClose} />
      <Toaster
        theme="dark"
        richColors
        position="bottom-left"
        toastOptions={toasterOptions}
      />
      <UpdaterDialog />
      <CloseBehaviorDialog
        open={isCloseDialogOpen}
        onOpenChange={setIsCloseDialogOpen}
        onConfirm={handleCloseConfirm}
      />

      <div className="flex-1 flex overflow-hidden">
        <Sidebar activeNav={activeNav} setActiveNav={setActiveNav} />

        {/* Main Content Area */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Views */}
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
