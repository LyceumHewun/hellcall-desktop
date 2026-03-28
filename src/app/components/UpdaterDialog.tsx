import { type MouseEvent, useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { Loader2, RefreshCw } from "lucide-react";
import ReactMarkdown from "react-markdown";
import { useTranslation } from "react-i18next";
import remarkGfm from "remark-gfm";
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
  const releaseNotes = updateInfo?.body?.trim();

  const handleReleaseNoteLinkClick = (
    event: MouseEvent<HTMLAnchorElement>,
    href?: string,
  ) => {
    if (!href || !/^(https?:|mailto:)/i.test(href)) {
      return;
    }

    event.preventDefault();
    void openUrl(href).catch((error) => {
      console.error("Failed to open release note link:", error);
    });
  };

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
          <div className="break-words leading-6">
            {releaseNotes ? (
              <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                components={{
                  h1: ({ children }) => (
                    <h1 className="mb-3 text-lg font-semibold text-white">
                      {children}
                    </h1>
                  ),
                  h2: ({ children }) => (
                    <h2 className="mb-3 text-base font-semibold text-white">
                      {children}
                    </h2>
                  ),
                  h3: ({ children }) => (
                    <h3 className="mb-2 text-sm font-semibold text-white">
                      {children}
                    </h3>
                  ),
                  p: ({ children }) => <p className="mb-3 last:mb-0">{children}</p>,
                  ul: ({ children }) => (
                    <ul className="mb-3 list-disc space-y-1 pl-5 last:mb-0">
                      {children}
                    </ul>
                  ),
                  ol: ({ children }) => (
                    <ol className="mb-3 list-decimal space-y-1 pl-5 last:mb-0">
                      {children}
                    </ol>
                  ),
                  li: ({ children }) => <li className="marker:text-zinc-400">{children}</li>,
                  a: ({ children, href }) => (
                    <a
                      href={href}
                      target="_blank"
                      rel="noreferrer"
                      onClick={(event) => handleReleaseNoteLinkClick(event, href)}
                      className="font-medium text-[#FCE100] underline underline-offset-4 transition-colors hover:text-[#fff27a]"
                    >
                      {children}
                    </a>
                  ),
                  blockquote: ({ children }) => (
                    <blockquote className="mb-3 border-l-2 border-[#FCE100]/50 pl-4 italic text-zinc-300 last:mb-0">
                      {children}
                    </blockquote>
                  ),
                  code: ({ children }) => (
                    <code className="rounded bg-white/10 px-1.5 py-0.5 font-mono text-[0.95em] text-zinc-100">
                      {children}
                    </code>
                  ),
                  pre: ({ children }) => (
                    <pre className="mb-3 overflow-x-auto rounded-md border border-white/10 bg-black/40 p-3 text-xs text-zinc-100 last:mb-0">
                      {children}
                    </pre>
                  ),
                  hr: () => <hr className="my-4 border-white/10" />,
                  table: ({ children }) => (
                    <div className="mb-3 overflow-x-auto last:mb-0">
                      <table className="min-w-full border-collapse text-left text-xs sm:text-sm">
                        {children}
                      </table>
                    </div>
                  ),
                  thead: ({ children }) => (
                    <thead className="border-b border-white/10 text-zinc-100">
                      {children}
                    </thead>
                  ),
                  tbody: ({ children }) => (
                    <tbody className="divide-y divide-white/5">{children}</tbody>
                  ),
                  th: ({ children }) => (
                    <th className="px-3 py-2 font-medium">{children}</th>
                  ),
                  td: ({ children }) => <td className="px-3 py-2 align-top">{children}</td>,
                }}
              >
                {releaseNotes}
              </ReactMarkdown>
            ) : (
              t("updater.no_release_notes")
            )}
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
