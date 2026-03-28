import { useEffect, useState } from "react";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { Loader2, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "./ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";

export function UpdaterDialog() {
  const { t } = useTranslation();
  const [updateInfo, setUpdateInfo] = useState<Update | null>(null);
  const [isInstalling, setIsInstalling] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    let updateResource: Update | null = null;

    const checkForUpdates = async () => {
      try {
        const update = await check();

        if (!update) {
          return;
        }

        if (cancelled) {
          await update.close().catch(() => undefined);
          return;
        }

        updateResource = update;
        setUpdateInfo(update);
      } catch (error) {
        console.error("Failed to check for updates:", error);
      }
    };

    void checkForUpdates();

    return () => {
      cancelled = true;

      if (updateResource) {
        void updateResource.close().catch(() => undefined);
      }
    };
  }, []);

  const handleIgnore = async () => {
    setErrorMessage(null);

    if (updateInfo) {
      await updateInfo.close().catch((error) => {
        console.error("Failed to close updater resource:", error);
      });
    }

    setUpdateInfo(null);
  };

  const handleInstall = async () => {
    if (!updateInfo) {
      return;
    }

    setIsInstalling(true);
    setErrorMessage(null);

    try {
      await updateInfo.downloadAndInstall();
      await relaunch();
    } catch (error) {
      console.error("Failed to install update:", error);
      setErrorMessage(
        error instanceof Error
          ? error.message
          : t("updater.install_failed_fallback"),
      );
      setIsInstalling(false);
    }
  };

  return (
    <Dialog
      open={updateInfo !== null}
      onOpenChange={(open) => {
        if (!open && !isInstalling) {
          void handleIgnore();
        }
      }}
    >
      <DialogContent className="border-zinc-800 bg-[#151922] text-white sm:max-w-xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2 text-xl">
            <RefreshCw className="h-5 w-5 text-[#FCE100]" />
            {updateInfo
              ? t("updater.title", { version: updateInfo.version })
              : t("updater.checking")}
          </DialogTitle>
          <DialogDescription className="text-zinc-400">
            {t("updater.description")}
          </DialogDescription>
        </DialogHeader>

        <div className="max-h-72 overflow-y-auto rounded-md border border-white/10 bg-black/20 p-4 text-sm text-zinc-200">
          <p className="mb-2 text-xs uppercase tracking-[0.2em] text-zinc-500">
            {t("updater.release_notes")}
          </p>
          <div className="whitespace-pre-wrap break-words leading-6">
            {updateInfo?.body?.trim() || t("updater.no_release_notes")}
          </div>
        </div>

        {errorMessage ? (
          <p className="text-sm text-red-300">{errorMessage}</p>
        ) : null}

        <DialogFooter>
          <Button
            variant="outline"
            onClick={() => void handleIgnore()}
            disabled={isInstalling}
            className="border-white/10 bg-transparent text-zinc-100 hover:bg-white/5 hover:text-white"
          >
            {t("updater.ignore")}
          </Button>
          <Button
            onClick={() => void handleInstall()}
            disabled={!updateInfo || isInstalling}
            className="bg-[#FCE100] text-black hover:bg-[#f7dd00]"
          >
            {isInstalling ? (
              <>
                <Loader2 className="h-4 w-4 animate-spin" />
                {t("updater.downloading")}
              </>
            ) : (
              t("updater.install_and_relaunch")
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
