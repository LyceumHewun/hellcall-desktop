import { useState } from "react";
import { Card, CardContent } from "../components/ui/card";
import { KeyRecorder, RustInput } from "../components/KeyRecorder";
import { KeySequence } from "../components/KeySequence";
import { useConfigStore } from "../../store/configStore";
import { useTranslation } from "react-i18next";

export function KeyBindingsView() {
  const { t } = useTranslation();
  const { config, updateConfig } = useConfigStore();

  const [activeRecordingAction, setActiveRecordingAction] = useState<
    string | null
  >(null);

  if (!config) return null;
  const bindings = config.key_map;

  const handleKeyRecorded = (action: string, newKeyCode: RustInput) => {
    updateConfig((c) => {
      c.key_map[action] = newKeyCode as any;
    });
    setActiveRecordingAction(null);
  };

  return (
    <>
      <div className="border-b border-white/10 p-6 shrink-0 bg-gradient-to-b from-[#0F1115] to-transparent backdrop-blur-sm">
        <div className="flex items-center justify-between">
          <div>
            <h1
              style={{ fontFamily: "var(--font-family-tech)" }}
              className="tracking-wider text-white mb-1 uppercase"
            >
              {t("bindings.title")}
            </h1>
            <p className="text-white/50 text-sm">{t("bindings.subtitle")}</p>
          </div>
        </div>
      </div>
      <div className="flex-1 overflow-y-auto p-6">
        <div className="max-w-3xl mx-auto space-y-6">
          <Card className="bg-[#1E2128] border-white/10">
            <CardContent className="p-0 divide-y divide-white/10 [&:last-child]:pb-0">
              {["UP", "DOWN", "LEFT", "RIGHT", "OPEN", "THROW", "RESEND"].map(
                (action) => {
                  const keyName = bindings[action];
                  if (!keyName) return null;
                  return (
                    <div
                      key={action}
                      className="flex items-center justify-between p-4 hover:bg-white/5 transition-colors"
                    >
                      <div className="flex items-center gap-3">
                        <div className="min-w-10">
                          <KeySequence sequence={[action]} compact />
                        </div>
                        <span className="text-white/50 font-mono text-sm">
                          {t(`bindings.action.${action.toLowerCase()}`)}
                        </span>
                      </div>
                      <KeyRecorder
                        actionName={action}
                        currentKeyCode={keyName as RustInput}
                        isRecording={activeRecordingAction === action}
                        onStartRecording={() =>
                          setActiveRecordingAction(action)
                        }
                        onKeyRecorded={(newKeyCode) =>
                          handleKeyRecorded(action, newKeyCode)
                        }
                        onCancelRecording={() => setActiveRecordingAction(null)}
                      />
                    </div>
                  );
                },
              )}
            </CardContent>
          </Card>
        </div>
      </div>
    </>
  );
}
