import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp } from 'lucide-react';

type Direction = 'up' | 'down' | 'left' | 'right';

interface KeySequenceProps {
  sequence?: Direction[];
  onEdit?: () => void;
  compact?: boolean;
}

const directionIcons = {
  up: ArrowUp,
  down: ArrowDown,
  left: ArrowLeft,
  right: ArrowRight,
};

export function KeySequence({ sequence = ['down', 'down', 'left', 'up', 'right'], onEdit, compact = false }: KeySequenceProps) {
  if (compact) {
    return (
      <div className="flex items-center gap-1.5">
        {sequence.map((direction, index) => {
          const Icon = directionIcons[direction];
          return (
            <div
              key={index}
              className="w-7 h-7 bg-[#0F1115] border border-[#FCE100] rounded flex items-center justify-center group"
              style={{
                boxShadow: '0 0 8px rgba(252, 225, 0, 0.15)',
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
        {sequence.map((direction, index) => {
          const Icon = directionIcons[direction];
          return (
            <div
              key={index}
              className="relative w-12 h-12 bg-gradient-to-b from-[#2A2D35] to-[#1E2128] rounded border border-[#FCE100]/50 flex items-center justify-center group"
              style={{
                boxShadow: '0 0 15px rgba(252, 225, 0, 0.2)',
              }}
            >
              <div
                className="absolute inset-0 bg-[#FCE100]/10 rounded opacity-0 group-hover:opacity-100 transition-opacity"
                style={{
                  animation: 'glow 1.5s ease-in-out infinite',
                }}
              />
              <Icon className="w-6 h-6 text-[#FCE100] relative z-10" strokeWidth={2.5} />
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
