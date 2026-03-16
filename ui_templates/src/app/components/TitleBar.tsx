import { Mic, X, Minus } from 'lucide-react';

export function TitleBar() {
  return (
    <div className="h-12 bg-[#0F1115] border-b border-white/10 flex items-center justify-between px-4 backdrop-blur-xl">
      <div className="flex items-center gap-2">
        <div className="relative">
          <Mic className="w-5 h-5 text-[#FCE100]" />
          <div className="absolute -bottom-1 -right-1 w-3 h-3 bg-[#FCE100] rounded-full flex items-center justify-center">
            <div className="w-1.5 h-1.5 bg-black rounded-full"></div>
          </div>
        </div>
        <span style={{ fontFamily: 'var(--font-family-tech)' }} className="tracking-wider text-white/90">
          HELLDIVERS TACTICAL MACRO
        </span>
      </div>

      <div className="flex items-center gap-1">
        <button className="w-12 h-8 flex items-center justify-center hover:bg-white/5 transition-colors">
          <Minus className="w-4 h-4 text-white/70" />
        </button>
        <button className="w-12 h-8 flex items-center justify-center hover:bg-[#D93A3A]/80 transition-colors">
          <X className="w-4 h-4 text-white/70" />
        </button>
      </div>
    </div>
  );
}
