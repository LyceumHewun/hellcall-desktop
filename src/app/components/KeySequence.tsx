import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp } from "lucide-react";

interface KeySequenceProps {
  sequence?: string[];
  onEdit?: () => void;
  compact?: boolean;
}

const directionIcons: Record<string, any> = {
  up: ArrowUp,
  down: ArrowDown,
  left: ArrowLeft,
  right: ArrowRight,
  UP: ArrowUp,
  DOWN: ArrowDown,
  LEFT: ArrowLeft,
  RIGHT: ArrowRight,
};

export function KeySequence({
  sequence = ["OPEN", "DOWN", "LEFT", "UP", "RIGHT", "THROW"],
  onEdit,
  compact = false,
}: KeySequenceProps) {
  if (compact) {
    return (
      <div className="flex items-center gap-1.5">
        {sequence.map((key, index) => {
          const upperKey = key.toUpperCase();
          if (upperKey === "OPEN") {
            return (
              <div
                key={index}
                className="px-1.5 h-7 bg-cyan-900 border border-cyan-500 text-cyan-50 rounded flex items-center justify-center text-[10px] font-bold"
              >
                OPN
              </div>
            );
          }
          if (upperKey === "THROW") {
            return (
              <div
                key={index}
                className="px-1.5 h-7 bg-red-900 border border-red-500 text-red-50 rounded flex items-center justify-center text-[10px] font-bold"
              >
                THR
              </div>
            );
          }

          if (!directionIcons[upperKey]) {
            return <div>{/* empty */}</div>;
          }

          const Icon = directionIcons[upperKey];
          return (
            <div
              key={index}
              className="w-7 h-7 bg-[#0F1115] border border-[#FCE100] rounded flex items-center justify-center group"
              style={{
                boxShadow: "0 0 8px rgba(252, 225, 0, 0.15)",
              }}
            >
              <Icon className="w-4 h-4 text-[#FCE100]" strokeWidth={2.5} />
            </div>
          );
        })}
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <label className="text-white/90">Macro Sequence</label>
        <button
          onClick={onEdit}
          className="px-3 py-1.5 text-sm border border-[#FCE100] text-[#FCE100] rounded hover:bg-[#FCE100]/10 transition-colors"
        >
          Record Sequence
        </button>
      </div>

      <div className="flex items-center gap-2 p-4 bg-black/30 rounded border border-white/5">
        {sequence.map((key, index) => {
          const upperKey = key.toUpperCase();

          if (upperKey === "OPEN") {
            return (
              <div
                key={index}
                className="px-3 h-12 bg-cyan-900 border border-cyan-500 text-cyan-50 rounded flex items-center justify-center text-sm font-bold shadow-[0_0_15px_rgba(6,182,212,0.2)]"
              >
                [ OPN ]
              </div>
            );
          }
          if (upperKey === "THROW") {
            return (
              <div
                key={index}
                className="px-3 h-12 bg-red-900 border border-red-500 text-red-50 rounded flex items-center justify-center text-sm font-bold shadow-[0_0_15px_rgba(239,68,68,0.2)]"
              >
                [ THR ]
              </div>
            );
          }

          if (!directionIcons[upperKey]) {
            return <div>{/* empty */}</div>;
          }

          const Icon = directionIcons[upperKey] || ArrowDown;
          return (
            <div
              key={index}
              className="relative w-12 h-12 bg-gradient-to-b from-[#2A2D35] to-[#1E2128] rounded border border-[#FCE100]/50 flex items-center justify-center group"
              style={{
                boxShadow: "0 0 15px rgba(252, 225, 0, 0.2)",
              }}
            >
              <div
                className="absolute inset-0 bg-[#FCE100]/10 rounded opacity-0 group-hover:opacity-100 transition-opacity"
                style={{
                  animation: "glow 1.5s ease-in-out infinite",
                }}
              />
              <Icon
                className="w-6 h-6 text-[#FCE100] relative z-10"
                strokeWidth={2.5}
              />
            </div>
          );
        })}
      </div>

      <style>{`
        @keyframes glow {
          0%, 100% { opacity: 0.3; }
          50% { opacity: 0.6; }
        }
      `}</style>
    </div>
  );
}
