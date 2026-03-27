import { AssetModelSelector } from "./AssetModelSelector";
import { useEngineStore } from "../../store/engineStore";
import { useTranslation } from "react-i18next";

function formatVisionModelName(id: string) {
  return id;
}

export function VisionModelSelector() {
  const { t } = useTranslation();
  const {
    selectedVisionModelId,
    setSelectedVisionModelId,
    setSelectedVisionModelReady,
  } = useEngineStore();

  return (
    <AssetModelSelector
      label={t("settings.vision_model")}
      placeholder={t("settings.vision_model_placeholder")}
      fetchCommand="get_available_vision_models"
      downloadCommand="download_vision_model"
      progressEventName="vision-download-progress"
      selectedModelId={selectedVisionModelId}
      setSelectedModelId={setSelectedVisionModelId}
      setSelectedModelReady={setSelectedVisionModelReady}
      formatModelName={formatVisionModelName}
    />
  );
}
