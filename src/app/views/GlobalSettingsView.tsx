import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Mic } from "lucide-react";
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
import { Button } from "../components/ui/button";
import { Progress } from "../components/ui/progress";
import { Slider } from "../components/ui/slider";
import { Label } from "../components/ui/label";
import { Input } from "../components/ui/input";
import { Switch } from "../components/ui/switch";
import { ModelSelector } from "../components/ModelSelector";
import { useConfigStore } from "../../store/configStore";
import { useEngineStore } from "../../store/engineStore";
import { useTranslation } from "react-i18next";

export function GlobalSettingsView() {
  const { config, updateConfig } = useConfigStore();
  const { t, i18n } = useTranslation();
  const { status, selectedDevice, setSelectedDevice } = useEngineStore();
  const isEngineRunning = status === "STARTING" || status === "ACTIVE";

  const [devices, setDevices] = useState<string[]>([]);
  const [isTestingMic, setIsTestingMic] = useState(false);
  const [micVolume, setMicVolume] = useState(0);

  const fetchDevices = async () => {
    try {
      const devs = await invoke<string[]>("get_audio_devices");
      setDevices(devs);
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    fetchDevices();
  }, []);

  useEffect(() => {
    let unlisten: () => void;
    if (isTestingMic) {
      listen<number>("mic_volume", (event) => {
        setMicVolume(Math.min(event.payload * 500, 100));
      }).then((fn) => {
        unlisten = fn;
      });
    } else {
      setMicVolume(0);
    }
    return () => {
      if (unlisten) unlisten();
    };
  }, [isTestingMic]);

  useEffect(() => {
    return () => {
      invoke("stop_mic_test").catch(console.error);
    };
  }, []);

  useEffect(() => {
    if (isEngineRunning && isTestingMic) {
      invoke("stop_mic_test").catch(console.error);
      setIsTestingMic(false);
    }
  }, [isEngineRunning, isTestingMic]);

  const toggleMicTest = async () => {
    if (isTestingMic) {
      await invoke("stop_mic_test");
      setIsTestingMic(false);
    } else {
      await invoke("start_mic_test", {
        deviceName: selectedDevice,
      });
      setIsTestingMic(true);
    }
  };

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
              <CardTitle className="text-[#FCE100] font-bold">
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
                  <SelectTrigger className="w-full bg-black/30 border-white/10 text-white">
                    <SelectValue placeholder="Language" />
                  </SelectTrigger>
                  <SelectContent className="bg-[#1E2128] border-white/10 text-white">
                    <SelectItem value="en">English</SelectItem>
                    <SelectItem value="zh">简体中文</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div className="space-y-2">
                <div className="flex items-center gap-2">
                  <Label>{t("settings.input_device")}</Label>
                </div>
                <Select
                  value={selectedDevice || "system_default"}
                  onValueChange={(val) =>
                    setSelectedDevice(val === "system_default" ? null : val)
                  }
                >
                  <SelectTrigger className="w-full bg-black/30 border-white/10 text-white">
                    <SelectValue placeholder="System Default" />
                  </SelectTrigger>
                  <SelectContent className="bg-[#1E2128] border-white/10 text-white">
                    <SelectItem value="system_default">
                      {t("settings.system_default")}
                    </SelectItem>
                    {devices.map((device) => (
                      <SelectItem key={device} value={device}>
                        {device}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              <div className="space-y-2">
                <div className="flex items-center gap-2">
                  <Label>{t("settings.mic_test")}</Label>
                  {isEngineRunning && (
                    <span className="-mt-0.5 text-xs text-yellow-500/80">
                      {t("settings.mic_test_disabled")}
                    </span>
                  )}
                </div>
                <div className="flex items-center gap-4">
                  <div className="flex-1 space-y-1.5 mt-1">
                    <Progress value={micVolume} className="h-2 bg-zinc-800" />
                    <div className="flex justify-between text-[10px] text-white/40 font-mono leading-none">
                      <span>0%</span>
                      <span>100%</span>
                    </div>
                  </div>
                  <Button
                    variant={isTestingMic ? "destructive" : "secondary"}
                    className={
                      isTestingMic
                        ? "w-32 cursor-pointer"
                        : "w-32 cursor-pointer text-white/70 border hover:border-primary/80 hover:bg-primary/10"
                    }
                    onClick={toggleMicTest}
                    disabled={isEngineRunning}
                  >
                    <Mic className="w-4 h-4 mr-2" />
                    {isTestingMic
                      ? t("settings.stop_test")
                      : t("settings.start_test")}
                  </Button>
                </div>
              </div>
            </CardContent>
          </Card>

          <Card className="bg-[#1E2128] border-white/10 text-white">
            <CardHeader>
              <CardTitle className="text-[#FCE100] font-bold">
                {t("settings.recognizer")}
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="space-y-3">
                <ModelSelector />
              </div>

              <div className="space-y-3">
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

              <div className="space-y-3">
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

              <div className="space-y-3">
                <Label>{t("settings.enable_denoise")}</Label>
                <div className="flex items-center justify-between space-x-4">
                  <p className="text-sm text-white/50">
                    {t("settings.enable_denoise_desc")}
                  </p>
                  <Switch
                    className="border cursor-pointer"
                    checked={config.recognizer.enable_denoise}
                    onCheckedChange={(checked) =>
                      updateConfig((c) => {
                        c.recognizer.enable_denoise = checked;
                      })
                    }
                  />
                </div>
              </div>
            </CardContent>
          </Card>

          <Card className="bg-[#1E2128] border-white/10 text-white">
            <CardHeader>
              <CardTitle className="text-[#FCE100] font-bold">
                {t("settings.trigger")}
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="space-y-2">
                <Label>
                  {t("settings.talk_mode", "Voice Recognition Mode")}
                </Label>
                <Select
                  value={config.recognizer.talk_mode || "voice_activation"}
                  onValueChange={(val) =>
                    updateConfig((c) => {
                      c.recognizer.talk_mode = val as any;
                    })
                  }
                >
                  <SelectTrigger className="w-full bg-black/30 border-white/10 text-white">
                    <SelectValue placeholder="Voice Activation" />
                  </SelectTrigger>
                  <SelectContent className="bg-[#1E2128] border-white/10 text-white">
                    <SelectItem value="voice_activation">
                      {t("settings.talk_mode_vad", "Voice Activation")}
                    </SelectItem>
                    <SelectItem value="push_to_talk">
                      {t("settings.talk_mode_ptt", "Push-to-Talk")}
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>

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
              <CardTitle className="text-[#FCE100] font-bold">
                {t("settings.vision")}
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="space-y-3">
                <Label>{t("settings.enable_occ")}</Label>
                <div className="flex items-center justify-between space-x-4">
                  <p className="text-sm text-white/50">
                    {t("settings.enable_occ_desc")}
                  </p>
                  <Switch
                    className="border cursor-pointer"
                    checked={config.vision?.enable_occ ?? true}
                    onCheckedChange={(checked) =>
                      updateConfig((c) => {
                        if (!c.vision) {
                          c.vision = {
                            enable_occ: checked,
                            capture_ratio: 0.5,
                          };
                        } else {
                          c.vision.enable_occ = checked;
                        }
                      })
                    }
                  />
                </div>
              </div>

              <div className="space-y-3">
                <Label>
                  {t("settings.capture_ratio", {
                    val: (config.vision?.capture_ratio ?? 0.8).toFixed(2),
                  })}
                </Label>
                <Slider
                  value={[config.vision?.capture_ratio ?? 0.8]}
                  min={0.1}
                  max={1.0}
                  step={0.05}
                  onValueChange={([val]) =>
                    updateConfig((c) => {
                      if (!c.vision) {
                        c.vision = { enable_occ: true, capture_ratio: val };
                      } else {
                        c.vision.capture_ratio = val;
                      }
                    })
                  }
                />
              </div>
            </CardContent>
          </Card>

          <Card className="bg-[#1E2128] border-white/10 text-white">
            <CardHeader>
              <CardTitle className="text-[#FCE100] font-bold">
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
        </div>
      </div>
    </>
  );
}
