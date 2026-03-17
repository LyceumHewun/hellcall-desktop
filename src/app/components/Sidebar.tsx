import { useState } from "react";
import { Settings, Keyboard, Command } from "lucide-react";

interface SidebarProps {
  activeNav: string;
  setActiveNav: (nav: string) => void;
}

export function Sidebar({ activeNav, setActiveNav }: SidebarProps) {
  const [isActive, setIsActive] = useState(false);

  return (
    <div className="w-64 bg-[#0F1115] border-r border-white/10 flex flex-col p-4 gap-6">
      {/* Status Toggle */}
      <button
        onClick={() => setIsActive(!isActive)}
        className={`relative overflow-hidden transition-all duration-300 ${
          isActive
            ? "bg-[#FCE100] border-[#FCE100] shadow-[0_0_20px_rgba(252,225,0,0.3)]"
            : "bg-transparent border-white/20"
        } border-2 rounded p-4 group`}
      >
        <div
          className={`absolute inset-0 transition-opacity duration-1000 ${
            isActive ? "opacity-100" : "opacity-0"
          }`}
          style={{
            background:
              "radial-gradient(circle at center, rgba(252, 225, 0, 0.2) 0%, transparent 70%)",
            animation: isActive ? "pulse 2s ease-in-out infinite" : "none",
          }}
        />
        <div className="relative flex flex-col items-center gap-2">
          <div
            className={`w-3 h-3 rounded-full ${isActive ? "bg-black" : "bg-[#FCE100]"}`}
          />
          <span
            style={{ fontFamily: "var(--font-family-tech)" }}
            className={`tracking-wider ${isActive ? "text-black" : "text-white/70"}`}
          >
            VOICE LINK
          </span>
          <span
            style={{ fontFamily: "var(--font-family-tech)" }}
            className={`tracking-wider ${isActive ? "text-black" : "text-white/50"}`}
          >
            {isActive ? "ACTIVE" : "OFFLINE"}
          </span>
        </div>
      </button>

      {/* Navigation */}
      <nav className="flex flex-col gap-1">
        <button
          onClick={() => setActiveNav("macros")}
          className={`flex items-center gap-3 px-4 py-3 rounded transition-colors ${
            activeNav === "macros"
              ? "bg-white/10 text-white"
              : "text-white/60 hover:bg-white/5 hover:text-white/80"
          }`}
        >
          <Command className="w-4 h-4" />
          <span>Stratagem Macros</span>
        </button>

        <button
          onClick={() => setActiveNav("keybindings")}
          className={`flex items-center gap-3 px-4 py-3 rounded transition-colors ${
            activeNav === "keybindings"
              ? "bg-white/10 text-white"
              : "text-white/60 hover:bg-white/5 hover:text-white/80"
          }`}
        >
          <Keyboard className="w-4 h-4" />
          <span>Key Bindings</span>
        </button>

        <button
          onClick={() => setActiveNav("settings")}
          className={`flex items-center gap-3 px-4 py-3 rounded transition-colors ${
            activeNav === "settings"
              ? "bg-white/10 text-white"
              : "text-white/60 hover:bg-white/5 hover:text-white/80"
          }`}
        >
          <Settings className="w-4 h-4" />
          <span>Global Settings</span>
        </button>
      </nav>

      <style>{`
        @keyframes pulse {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.6; }
        }
      `}</style>
    </div>
  );
}
