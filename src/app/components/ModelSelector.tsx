import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { CheckCircle2, Download } from "lucide-react";
import { toast } from "sonner";
import { Button } from "./ui/button";
import { Label } from "./ui/label";
import { Progress } from "./ui/progress";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./ui/select";
import { useEngineStore } from "../../store/engineStore";
import { useTranslation } from "react-i18next";

type BackendVoskModel = {
  id: string;
  url: string;
  is_downloaded: boolean;
};

type VoskModel = BackendVoskModel & {
  name: string;
};

type ModelDownloadProgressPayload = {
  id: string;
  progress: number;
  status: string;
};

function formatModelName(id: string) {
  return id.replace(/^vosk-model-small-/, "");
}

export function ModelSelector() {
  const { t } = useTranslation();
  const {
    selectedVoskModelId,
    setSelectedVoskModelId,
    setSelectedVoskModelReady,
  } = useEngineStore();
  const [models, setModels] = useState<VoskModel[]>([]);
  const [downloadingId, setDownloadingId] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [downloadStatus, setDownloadStatus] = useState("");
  const [isLoading, setIsLoading] = useState(true);

  const fetchModels = async () => {
    try {
      setIsLoading(true);
      const fetchedModels = await invoke<BackendVoskModel[]>(
        "get_available_vosk_models",
      );
      const nextModels = fetchedModels.map((model) => ({
        ...model,
        name: formatModelName(model.id),
      }));

      setModels(nextModels);

      if (nextModels.length === 0) {
        setSelectedVoskModelReady(false);
        return;
      }

      const currentSelectionExists = nextModels.some(
        (model) => model.id === selectedVoskModelId,
      );

      if (!currentSelectionExists) {
        setSelectedVoskModelId(nextModels[0].id);
        setSelectedVoskModelReady(nextModels[0].is_downloaded);
        return;
      }

      const selectedModel = nextModels.find(
        (model) => model.id === selectedVoskModelId,
      );
      setSelectedVoskModelReady(Boolean(selectedModel?.is_downloaded));
    } catch (error) {
      console.error("Failed to load Vosk models:", error);
      setSelectedVoskModelReady(false);
      toast.error(t("settings.model_fetch_failed"));
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    fetchModels();
  }, []);

  useEffect(() => {
    if (isLoading || models.length === 0) {
      return;
    }

    const selectedModel = models.find(
      (model) => model.id === selectedVoskModelId,
    );
    setSelectedVoskModelReady(Boolean(selectedModel?.is_downloaded));
  }, [isLoading, models, selectedVoskModelId, setSelectedVoskModelReady]);

  useEffect(() => {
    let unlistenFn: UnlistenFn | null = null;
    let disposed = false;

    const unlistenPromise = listen<ModelDownloadProgressPayload>(
      "model-download-progress",
      async (event) => {
        const payload = event.payload;
        setDownloadingId(payload.id);
        setDownloadProgress(payload.progress);
        setDownloadStatus(payload.status);

        if (payload.status === "Complete") {
          await fetchModels();
          setDownloadingId(null);
          setDownloadProgress(0);
          toast.success(t("settings.model_download_complete"));
        } else if (payload.status.startsWith("Failed:")) {
          await fetchModels();
          setDownloadingId(null);
          setDownloadProgress(0);
          toast.error(payload.status);
        }
      },
    ).then((fn) => {
      if (disposed) {
        fn();
        return fn;
      }
      unlistenFn = fn;
      return fn;
    });

    return () => {
      disposed = true;
      if (unlistenFn) {
        unlistenFn();
      } else {
        unlistenPromise.then((fn) => fn());
      }
    };
  }, [t]);

  const selectedModel = useMemo(
    () => models.find((model) => model.id === selectedVoskModelId) ?? null,
    [models, selectedVoskModelId],
  );

  const isDownloadingSelectedModel = selectedModel
    ? downloadingId === selectedModel.id
    : false;
  const isSelectDisabled = isLoading || Boolean(downloadingId);

  const handleDownload = async () => {
    if (!selectedModel) {
      return;
    }

    const modelToDownload = selectedModel;

    setDownloadingId(modelToDownload.id);
    setDownloadProgress(0);
    setDownloadStatus(t("settings.model_download_starting"));

    try {
      await invoke("download_vosk_model", {
        modelId: modelToDownload.id,
        url: modelToDownload.url,
      });
    } catch (error) {
      const message =
        error instanceof Error
          ? error.message
          : String(error ?? "Unknown error");
      setDownloadingId(null);
      setDownloadProgress(0);
      setDownloadStatus(message);
      toast.error(message);
    }
  };

  return (
    <div className="space-y-3">
      <Label>{t("settings.vosk_model")}</Label>
      <div className="flex flex-col gap-3 xl:flex-row xl:items-center">
        <div className="min-w-0 flex-1">
          <Select
            value={selectedVoskModelId}
            onValueChange={setSelectedVoskModelId}
            disabled={isSelectDisabled}
          >
            <SelectTrigger className="w-full bg-black/30 border-white/10 text-white disabled:opacity-60">
              <SelectValue placeholder={t("settings.vosk_model_placeholder")} />
            </SelectTrigger>
            <SelectContent className="bg-[#1E2128] border-white/10 text-white">
              {models.map((model) => (
                <SelectItem key={model.id} value={model.id}>
                  {model.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="xl:w-64">
          {isDownloadingSelectedModel ? (
            <div className="rounded-md border border-white/10 bg-black/20 px-3 py-2">
              <div className="mb-2 flex items-center justify-between text-xs text-white/70">
                <span>{downloadStatus}</span>
                <span>{downloadProgress}%</span>
              </div>
              <Progress value={downloadProgress} className="h-2 bg-zinc-800" />
            </div>
          ) : selectedModel && !selectedModel.is_downloaded ? (
            <Button
              variant="outline"
              className="w-full cursor-pointer border-white/15 bg-black/20 text-white hover:border-[#FCE100]/50 hover:bg-[#FCE100]/10 hover:text-white"
              onClick={handleDownload}
              disabled={Boolean(downloadingId) || isLoading}
            >
              <Download className="w-4 h-4" />
              {t("settings.download_model")}
            </Button>
          ) : selectedModel ? (
            <div className="flex h-9 items-center justify-center gap-2 rounded-md border border-[#FCE100]/50 bg-[#FCE100]/10 px-3 text-sm text-white/70">
              <CheckCircle2 className="w-4 h-4" />
              <span>{t("settings.model_ready")}</span>
            </div>
          ) : (
            <div className="flex h-9 items-center rounded-md border border-white/10 bg-black/20 px-3 text-sm text-white/50">
              {t("settings.model_loading")}
            </div>
          )}
        </div>
      </div>

      {selectedModel?.url ? (
        <p className="text-xs text-white/45">{selectedModel.url}</p>
      ) : null}

      {downloadStatus &&
      !isDownloadingSelectedModel &&
      downloadingId === null ? (
        <p
          className={`text-xs ${
            downloadStatus.startsWith("Failed:")
              ? "text-red-400"
              : "text-white/55"
          }`}
        >
          {downloadStatus}
        </p>
      ) : null}
    </div>
  );
}
