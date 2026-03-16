import { useState } from 'react';
import { Card, CardContent } from '../components/ui/card';
import { Button } from '../components/ui/button';

export function KeyBindingsView() {
  const [bindings, setBindings] = useState({
    UP: 'UpArrow',
    DOWN: 'DownArrow',
    LEFT: 'LeftArrow',
    RIGHT: 'RightArrow',
    OPEN: 'ControlLeft',
    RESEND: 'KeyR',
    THROW: 'Space',
  });

  const [activeKey, setActiveKey] = useState<string | null>(null);

  const handleKeyClick = (action: string) => {
    setActiveKey(action);
  };

  return (
    <div className="flex-1 overflow-y-auto p-6">
      <div className="max-w-3xl mx-auto space-y-6">
        <h2 className="text-xl font-bold text-white mb-4">Key Bindings</h2>
        <Card className="bg-[#1E2128] border-white/10">
          <CardContent className="p-0 divide-y divide-white/10">
            {Object.entries(bindings).map(([action, keyName]) => (
              <div key={action} className="flex items-center justify-between p-4 hover:bg-white/5 transition-colors">
                <span className="text-white/50 font-mono text-sm">{action}</span>
                <Button
                  variant="outline"
                  className={activeKey === action ? "bg-[#FCE100] text-black hover:bg-[#FCE100]/90 border-[#FCE100]" : "bg-black/30 border-white/10 text-white hover:bg-white/10 hover:text-white"}
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
  );
}
