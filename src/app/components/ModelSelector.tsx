import { AssetModelSelector } from "./AssetModelSelector";
import { useEngineStore } from "../../store/engineStore";
import { useTranslation } from "react-i18next";

function formatVoskModelName(id: string) {
  return id.replace(/^vosk-model-small-/, "");
}

export function ModelSelector() {
  const { t } = useTranslation();
  const {
    selectedVoskModelId,
    setSelectedVoskModelId,
    setSelectedVoskModelReady,
  } = useEngineStore();

  return (
    <AssetModelSelector
      label={t("settings.vosk_model")}
      placeholder={t("settings.vosk_model_placeholder")}
      fetchCommand="get_available_vosk_models"
      downloadCommand="download_vosk_model"
      progressEventName="model-download-progress"
      selectedModelId={selectedVoskModelId}
      setSelectedModelId={setSelectedVoskModelId}
      setSelectedModelReady={setSelectedVoskModelReady}
      formatModelName={formatVoskModelName}
    />
  );
}
