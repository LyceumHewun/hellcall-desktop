import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "../components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../components/ui/select";
import { Slider } from "../components/ui/slider";
import { Label } from "../components/ui/label";
import { Input } from "../components/ui/input";
import { Switch } from "../components/ui/switch";
import { useConfigStore } from "../../store/configStore";
import { useTranslation } from "react-i18next";

export function GlobalSettingsView() {
  const { config, updateConfig } = useConfigStore();
  const { t, i18n } = useTranslation();

  if (!config) return null;

  return (
    <>
      <div className="border-b border-white/10 p-6 shrink-0 bg-gradient-to-b from-[#0F1115] to-transparent backdrop-blur-sm">
        <div className="flex items-center justify-between">
          <div>
            <h1
              style={{ fontFamily: "var(--font-family-tech)" }}
              className="tracking-wider text-white mb-1 uppercase"
            >
              {t("settings.title")}
            </h1>
            <p className="text-white/50 text-sm">{t("settings.subtitle")}</p>
          </div>
        </div>
      </div>
      <div className="flex-1 overflow-y-auto p-6 space-y-6">
        <div className="max-w-4xl mx-auto space-y-6 text-white">
          <Card className="bg-[#1E2128] border-white/10 text-white">
            <CardHeader>
              <CardTitle className="text-[#FCE100]">
                {t("settings.app_preferences")}
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="space-y-2">
                <Label>{t("settings.language")}</Label>
                <Select
                  value={i18n.language.startsWith("zh") ? "zh" : "en"}
                  onValueChange={(val) => i18n.changeLanguage(val)}
                >
                  <SelectTrigger className="w-[180px] bg-black/30 border-white/10 text-white">
                    <SelectValue placeholder="Language" />
                  </SelectTrigger>
                  <SelectContent className="bg-[#1E2128] border-white/10 text-white">
                    <SelectItem value="en">English</SelectItem>
                    <SelectItem value="zh">简体中文</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </CardContent>
          </Card>

          <Card className="bg-[#1E2128] border-white/10 text-white">
            <CardHeader>
              <CardTitle className="text-[#FCE100]">
                {t("settings.trigger")}
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="space-y-2">
                <Label>{t("settings.hit_word")}</Label>
                <Input
                  className="bg-black/30 border-white/10"
                  value={config.trigger.hit_word || ""}
                  onChange={(e) =>
                    updateConfig((c) => {
                      c.trigger.hit_word = e.target.value;
                    })
                  }
                />
              </div>
              <div className="space-y-2">
                <Label>{t("settings.hit_word_grammar")}</Label>
                <Input
                  className="bg-black/30 border-white/10"
                  value={config.trigger.hit_word_grammar || ""}
                  onChange={(e) =>
                    updateConfig((c) => {
                      c.trigger.hit_word_grammar = e.target.value;
                    })
                  }
                />
              </div>
            </CardContent>
          </Card>

          <Card className="bg-[#1E2128] border-white/10 text-white">
            <CardHeader>
              <CardTitle className="text-[#FCE100]">
                {t("settings.key_presser")}
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="space-y-2">
                <Label>{t("settings.wait_open_time")}</Label>
                <Input
                  type="number"
                  className="bg-black/30 border-white/10"
                  value={config.key_presser.wait_open_time}
                  onChange={(e) =>
                    updateConfig((c) => {
                      c.key_presser.wait_open_time = Number(e.target.value);
                    })
                  }
                />
              </div>
              <div className="space-y-2">
                <Label>{t("settings.key_release_interval")}</Label>
                <Input
                  type="number"
                  className="bg-black/30 border-white/10"
                  value={config.key_presser.key_release_interval}
                  onChange={(e) =>
                    updateConfig((c) => {
                      c.key_presser.key_release_interval = Number(
                        e.target.value,
                      );
                    })
                  }
                />
              </div>
              <div className="space-y-2">
                <Label>{t("settings.diff_key_interval")}</Label>
                <Input
                  type="number"
                  className="bg-black/30 border-white/10"
                  value={config.key_presser.diff_key_interval}
                  onChange={(e) =>
                    updateConfig((c) => {
                      c.key_presser.diff_key_interval = Number(e.target.value);
                    })
                  }
                />
              </div>
            </CardContent>
          </Card>

          <Card className="bg-[#1E2128] border-white/10 text-white">
            <CardHeader>
              <CardTitle className="text-[#FCE100]">
                {t("settings.recognizer")}
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="space-y-2">
                <Label>
                  {t("settings.chunk_time", {
                    val: config.recognizer.chunk_time,
                  })}
                </Label>
                <Slider
                  value={[config.recognizer.chunk_time]}
                  min={0.12}
                  max={1.0}
                  step={0.02}
                  onValueChange={([val]) =>
                    updateConfig((c) => {
                      c.recognizer.chunk_time = val;
                    })
                  }
                />
              </div>
              <div className="space-y-2">
                <Label>
                  {t("settings.vad_silence", {
                    val: config.recognizer.vad_silence_duration,
                  })}
                </Label>
                <Slider
                  value={[config.recognizer.vad_silence_duration]}
                  min={50}
                  max={500}
                  step={10}
                  onValueChange={([val]) =>
                    updateConfig((c) => {
                      c.recognizer.vad_silence_duration = val;
                    })
                  }
                />
              </div>
              <div className="flex flex-row items-center justify-between bg-black/30 rounded-lg border border-zinc-800 p-4 mt-4">
                <div className="space-y-0.5">
                  <Label>{t("settings.enable_denoise")}</Label>
                  <p className="text-sm text-muted-foreground text-white/50">
                    {t(
                      "settings.enable_denoise_desc",
                      "Filters out background noise (e.g., mechanical keyboards) to improve voice recognition accuracy.",
                    )}
                  </p>
                </div>
                <Switch
                  checked={config.recognizer.enable_denoise}
                  onCheckedChange={(checked) =>
                    updateConfig((c) => {
                      c.recognizer.enable_denoise = checked;
                    })
                  }
                />
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </>
  );
}
