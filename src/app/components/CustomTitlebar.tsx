import { Mic, Minus, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";

export function CustomTitlebar() {
  const appWindow = getCurrentWindow();

  const handleMinimize = () => appWindow.minimize();
  const handleClose = () => appWindow.close();

  return (
    <div
      data-tauri-drag-region
      className="h-12 flex border-b border-white/10 justify-between items-center bg-transparent select-none shrink-0"
    >
      <div
        className="flex items-center gap-2 px-4 h-full flex-1 cursor-default"
        data-tauri-drag-region
      >
        <div className="relative pointer-events-none">
          <Mic className="w-5 h-5 text-[#FCE100]" />
          <div className="absolute -bottom-1 -right-1 w-3 h-3 bg-[#FCE100] rounded-full flex items-center justify-center">
            <div className="w-1.5 h-1.5 bg-black rounded-full"></div>
          </div>
        </div>
        <span
          style={{ fontFamily: "var(--font-family-tech)" }}
          className="tracking-wider font-bold text-white/90 pointer-events-none"
        >
          HELLDIVERS TACTICAL MACRO
        </span>
      </div>

      <div className="flex h-full">
        <button
          onClick={handleMinimize}
          className="w-12 h-full flex items-center justify-center hover:bg-white/5 transition-colors bg-transparent"
        >
          <Minus className="w-4 h-4 text-white/70 pointer-events-none" />
        </button>
        <button
          onClick={handleClose}
          className="w-12 h-full flex items-center justify-center hover:bg-red-600 transition-colors bg-transparent hover:text-white"
        >
          <X className="w-4 h-4 text-white/70 pointer-events-none" />
        </button>
      </div>
    </div>
  );
}
