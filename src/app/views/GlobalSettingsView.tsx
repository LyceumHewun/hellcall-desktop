import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "../components/ui/card";
import { Slider } from "../components/ui/slider";
import { Label } from "../components/ui/label";
import { Input } from "../components/ui/input";
import { useConfigStore } from "../../store/configStore";

export function GlobalSettingsView() {
  const { config, updateConfig } = useConfigStore();

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
              GLOBAL CONFIGURATION
            </h1>
            <p className="text-white/50 text-sm">
              Tweak voice recognition and presser engine behaviors
            </p>
          </div>
        </div>
      </div>
      <div className="flex-1 overflow-y-auto p-6 space-y-6">
        <div className="max-w-4xl mx-auto space-y-6 text-white">
          <Card className="bg-[#1E2128] border-white/10 text-white">
            <CardHeader>
              <CardTitle className="text-[#FCE100]">Recognizer</CardTitle>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="space-y-2">
                <Label>Chunk Time ({config.recognizer.chunk_time})</Label>
                <Slider
                  value={[config.recognizer.chunk_time]}
                  min={0.1}
                  max={1.0}
                  step={0.1}
                  onValueChange={([val]) =>
                    updateConfig((c) => {
                      c.recognizer.chunk_time = val;
                    })
                  }
                />
              </div>
              <div className="space-y-2">
                <Label>
                  VAD Silence Duration ({config.recognizer.vad_silence_duration}
                  )
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
            </CardContent>
          </Card>

          <Card className="bg-[#1E2128] border-white/10 text-white">
            <CardHeader>
              <CardTitle className="text-[#FCE100]">Key Presser</CardTitle>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="space-y-2">
                <Label>Wait Open Time</Label>
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
                <Label>Key Release Interval</Label>
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
                <Label>Diff Key Interval</Label>
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
              <CardTitle className="text-[#FCE100]">Trigger</CardTitle>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="space-y-2">
                <Label>Hit Word</Label>
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
                <Label>Hit Word Grammar</Label>
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
        </div>
      </div>
    </>
  );
}
