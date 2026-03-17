import { useState } from "react";
import { Card, CardContent } from "../components/ui/card";
import { KeyRecorder, RustInput } from "../components/KeyRecorder";
import { KeySequence } from "../components/KeySequence";

export function KeyBindingsView() {
  const [bindings, setBindings] = useState<Record<string, RustInput>>({
    UP: "UpArrow",
    DOWN: "DownArrow",
    LEFT: "LeftArrow",
    RIGHT: "RightArrow",
    OPEN: "ControlLeft",
    THROW: "Space",
    RESEND: "KeyR",
  });

  const [activeRecordingAction, setActiveRecordingAction] = useState<
    string | null
  >(null);

  const handleKeyRecorded = (action: string, newKeyCode: RustInput) => {
    setBindings((prev) => ({
      ...prev,
      [action]: newKeyCode,
    }));
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
              HARDWARE KEY BINDINGS
            </h1>
            <p className="text-white/50 text-sm">
              Map logical commands to physical keyboard inputs
            </p>
          </div>
        </div>
      </div>
      <div className="flex-1 overflow-y-auto p-6">
        <div className="max-w-3xl mx-auto space-y-6">
          <Card className="bg-[#1E2128] border-white/10">
            <CardContent className="p-0 divide-y divide-white/10 [&:last-child]:pb-0">
              {Object.entries(bindings).map(([action, keyName]) => (
                <div
                  key={action}
                  className="flex items-center justify-between p-4 hover:bg-white/5 transition-colors"
                >
                  <div className="flex items-center gap-3">
                    <span className="min-w-12 text-white/50 font-mono text-sm">
                      {action}
                    </span>
                    <KeySequence sequence={[action]} compact />
                  </div>
                  <KeyRecorder
                    actionName={action}
                    currentKeyCode={keyName}
                    isRecording={activeRecordingAction === action}
                    onStartRecording={() => setActiveRecordingAction(action)}
                    onKeyRecorded={(newKeyCode) =>
                      handleKeyRecorded(action, newKeyCode)
                    }
                    onCancelRecording={() => setActiveRecordingAction(null)}
                  />
                </div>
              ))}
            </CardContent>
          </Card>
        </div>
      </div>
    </>
  );
}
