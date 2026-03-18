import { useEffect, useRef } from "react";
import { Button } from "./ui/button";

export type RustInput = string | { Unknown: number };

export function mapWebEventToRustInput(
  event: KeyboardEvent | MouseEvent,
): RustInput {
  if (event.type === "mousedown") {
    const mouseEvent = event as MouseEvent;
    switch (mouseEvent.button) {
      case 0:
        return "Left";
      case 1:
        return "Middle";
      case 2:
        return "Right";
      case 3:
        return { Unknown: 1 };
      case 4:
        return { Unknown: 2 };
      default:
        return "Left";
    }
  }

  const keyboardEvent = event as KeyboardEvent;
  const webCode = keyboardEvent.code;
  switch (webCode) {
    case "AltLeft":
      return "Alt";
    case "AltRight":
      return "AltGr";
    case "Backspace":
      return "Backspace";
    case "CapsLock":
      return "CapsLock";
    case "ControlLeft":
      return "ControlLeft";
    case "ControlRight":
      return "ControlRight";
    case "Delete":
      return "Delete";
    case "ArrowDown":
      return "DownArrow";
    case "End":
      return "End";
    case "Escape":
      return "Escape";
    case "Home":
      return "Home";
    case "ArrowLeft":
      return "LeftArrow";
    case "MetaLeft":
      return "MetaLeft";
    case "MetaRight":
      return "MetaRight";
    case "PageDown":
      return "PageDown";
    case "PageUp":
      return "PageUp";
    case "Enter":
      return "Return";
    case "ArrowRight":
      return "RightArrow";
    case "ShiftLeft":
      return "ShiftLeft";
    case "ShiftRight":
      return "ShiftRight";
    case "Space":
      return "Space";
    case "Tab":
      return "Tab";
    case "ArrowUp":
      return "UpArrow";
    case "PrintScreen":
      return "PrintScreen";
    case "ScrollLock":
      return "ScrollLock";
    case "Pause":
      return "Pause";
    case "NumLock":
      return "NumLock";
    case "Backquote":
      return "BackQuote";
    case "Minus":
      return "Minus";
    case "Equal":
      return "Equal";
    case "BracketLeft":
      return "LeftBracket";
    case "BracketRight":
      return "RightBracket";
    case "Semicolon":
      return "SemiColon";
    case "Quote":
      return "Quote";
    case "Backslash":
      return "BackSlash";
    case "IntlBackslash":
      return "IntlBackslash";
    case "Comma":
      return "Comma";
    case "Period":
      return "Dot";
    case "Slash":
      return "Slash";
    case "Insert":
      return "Insert";
    case "NumpadEnter":
      return "KpReturn";
    case "NumpadSubtract":
      return "KpMinus";
    case "NumpadAdd":
      return "KpPlus";
    case "NumpadMultiply":
      return "KpMultiply";
    case "NumpadDivide":
      return "KpDivide";
    case "NumpadDecimal":
      return "KpDelete";
    case "Fn":
      return "Function";
    default:
      if (webCode.startsWith("Digit")) {
        return webCode.replace("Digit", "Num");
      }
      if (webCode.startsWith("Numpad")) {
        return webCode.replace("Numpad", "Kp");
      }
      return webCode;
  }
}

export function formatKeyCode(keyCode: RustInput): string {
  if (typeof keyCode === "string") return keyCode;
  if (keyCode && typeof keyCode === "object" && "Unknown" in keyCode) {
    if (keyCode.Unknown === 1) return "Mouse X1";
    if (keyCode.Unknown === 2) return "Mouse X2";
    return `Unknown(${keyCode.Unknown})`;
  }
  return "Unknown";
}

interface KeyRecorderProps {
  actionName: string;
  currentKeyCode: RustInput;
  isRecording: boolean;
  onStartRecording: () => void;
  onKeyRecorded: (newKeyCode: RustInput) => void;
  onCancelRecording: () => void;
  onClear?: () => void;
}

export function KeyRecorder({
  actionName,
  currentKeyCode,
  isRecording,
  onStartRecording,
  onKeyRecorded,
  onCancelRecording,
  onClear,
}: KeyRecorderProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!isRecording) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();

      if (event.code === "Escape") {
        onCancelRecording();
        return;
      }

      if (event.code === "Delete" || event.code === "Backspace") {
        if (onClear) {
          onClear();
        }
        onCancelRecording();
        return;
      }

      onKeyRecorded(mapWebEventToRustInput(event));
    };

    const handleMouseDown = (event: MouseEvent) => {
      event.preventDefault();
      event.stopPropagation();
      onKeyRecorded(mapWebEventToRustInput(event));
    };

    const handleContextMenu = (event: MouseEvent) => {
      event.preventDefault();
    };

    // Use setTimeout to avoid triggering on the same click that activated it
    const timeoutId = setTimeout(() => {
      window.addEventListener("keydown", handleKeyDown, { capture: true });
      window.addEventListener("mousedown", handleMouseDown, { capture: true });
      window.addEventListener("contextmenu", handleContextMenu, {
        capture: true,
      });
    }, 0);

    return () => {
      clearTimeout(timeoutId);
      window.removeEventListener("keydown", handleKeyDown, { capture: true });
      window.removeEventListener("mousedown", handleMouseDown, {
        capture: true,
      });
      window.removeEventListener("contextmenu", handleContextMenu, {
        capture: true,
      });
    };
  }, [isRecording, onKeyRecorded, onCancelRecording]);

  return (
    <div ref={containerRef} className="inline-block">
      <Button
        aria-label={`Record key for ${actionName}`}
        variant="outline"
        className={
          isRecording
            ? "bg-[#FCE100] text-black font-bold animate-pulse hover:bg-[#FCE100]/90 border-[#FCE100] min-w-[120px]"
            : "bg-zinc-800 border-white/10 text-white hover:bg-zinc-700 hover:text-white min-w-[120px]"
        }
        onClick={onStartRecording}
      >
        {isRecording ? "Listening..." : formatKeyCode(currentKeyCode)}
      </Button>
    </div>
  );
}
