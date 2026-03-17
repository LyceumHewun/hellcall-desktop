import { useState } from "react";
import { Card, CardContent } from "../components/ui/card";
import { Button } from "../components/ui/button";

export function KeyBindingsView() {
  const [bindings, setBindings] = useState({
    UP: "UpArrow",
    DOWN: "DownArrow",
    LEFT: "LeftArrow",
    RIGHT: "RightArrow",
    OPEN: "ControlLeft",
    RESEND: "KeyR",
    THROW: "Space",
  });

  const [activeKey, setActiveKey] = useState<string | null>(null);

  const handleKeyClick = (action: string) => {
    setActiveKey(action);
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
            <CardContent className="p-0 divide-y divide-white/10">
              {Object.entries(bindings).map(([action, keyName]) => (
                <div
                  key={action}
                  className="flex items-center justify-between p-4 hover:bg-white/5 transition-colors"
                >
                  <span className="text-white/50 font-mono text-sm">
                    {action}
                  </span>
                  <Button
                    variant="outline"
                    className={
                      activeKey === action
                        ? "bg-[#FCE100] text-black hover:bg-[#FCE100]/90 border-[#FCE100]"
                        : "bg-black/30 border-white/10 text-white hover:bg-white/10 hover:text-white"
                    }
                    onClick={() => handleKeyClick(action)}
                  >
                    {activeKey === action ? "Press any key..." : keyName}
                  </Button>
                </div>
              ))}
            </CardContent>
          </Card>
        </div>
      </div>
    </>
  );
}
