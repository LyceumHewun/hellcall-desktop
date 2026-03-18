import { useState, useEffect, useRef, useCallback } from "react";
import { attachLogger, LogLevel } from "@tauri-apps/plugin-log";

interface LogEntry {
  id: number;
  level: LogLevel;
  message: string;
  timestamp: Date;
}

const MAX_LOGS = 500;

let logIdCounter = 0;

function getLevelLabel(level: LogLevel): string {
  switch (level) {
    case LogLevel.Trace:
      return "TRACE";
    case LogLevel.Debug:
      return "DEBUG";
    case LogLevel.Info:
      return "INFO";
    case LogLevel.Warn:
      return "WARN";
    case LogLevel.Error:
      return "ERROR";
    default:
      return "UNKNOWN";
  }
}

function getLevelColor(level: LogLevel): string {
  switch (level) {
    case LogLevel.Trace:
      return "text-zinc-600";
    case LogLevel.Debug:
      return "text-zinc-500";
    case LogLevel.Info:
      return "text-zinc-300";
    case LogLevel.Warn:
      return "text-yellow-500";
    case LogLevel.Error:
      return "text-red-500";
    default:
      return "text-zinc-300";
  }
}

function formatTime(date: Date): string {
  const h = String(date.getHours()).padStart(2, "0");
  const m = String(date.getMinutes()).padStart(2, "0");
  const s = String(date.getSeconds()).padStart(2, "0");
  return `${h}:${m}:${s}`;
}

export function LogView() {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const bottomRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [isAutoScroll, setIsAutoScroll] = useState(true);

  const handleScroll = useCallback(() => {
    if (!containerRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
    // Consider auto-scroll active if within 60px of bottom
    setIsAutoScroll(scrollHeight - scrollTop - clientHeight < 60);
  }, []);

  useEffect(() => {
    const detachPromise = attachLogger(({ level, message }) => {
      const entry: LogEntry = {
        id: logIdCounter++,
        level,
        message,
        timestamp: new Date(),
      };
      setLogs((prev) => {
        const next = [...prev, entry];
        if (next.length > MAX_LOGS) {
          return next.slice(next.length - MAX_LOGS);
        }
        return next;
      });
    });

    return () => {
      detachPromise.then((detach) => detach());
    };
  }, []);

  useEffect(() => {
    if (isAutoScroll) {
      bottomRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [logs, isAutoScroll]);

  return (
    <div
      ref={containerRef}
      onScroll={handleScroll}
      className="h-full w-full bg-[#09090b] text-zinc-300 font-mono text-sm overflow-y-auto"
    >
      {/* Scanline overlay for terminal effect */}
      <div
        className="pointer-events-none fixed inset-0 z-10 opacity-[0.03]"
        style={{
          backgroundImage:
            "repeating-linear-gradient(0deg, transparent, transparent 2px, rgba(0,0,0,0.3) 2px, rgba(0,0,0,0.3) 4px)",
        }}
      />

      <div className="relative z-20 p-4">
        {logs.length === 0 && (
          <div className="flex items-center gap-2 text-zinc-600">
            <span className="inline-block w-2 h-4 bg-zinc-600 animate-pulse" />
            <span>Waiting for logs...</span>
          </div>
        )}

        {logs.map((entry) => (
          <div
            key={entry.id}
            className={`leading-6 ${getLevelColor(entry.level)}`}
          >
            <span className="text-zinc-600">
              [{formatTime(entry.timestamp)}]
            </span>{" "}
            <span
              className={`font-bold ${getLevelColor(entry.level)}`}
              style={{ minWidth: "5ch", display: "inline-block" }}
            >
              [{getLevelLabel(entry.level)}]
            </span>{" "}
            <span>{entry.message}</span>
          </div>
        ))}

        <div ref={bottomRef} />
      </div>

      {!isAutoScroll && logs.length > 0 && (
        <button
          onClick={() => {
            bottomRef.current?.scrollIntoView({ behavior: "smooth" });
            setIsAutoScroll(true);
          }}
          className="fixed bottom-6 right-6 z-30 bg-zinc-800 hover:bg-zinc-700 text-zinc-300 text-xs font-mono px-3 py-2 rounded border border-zinc-700 transition-colors shadow-lg"
        >
          ↓ Scroll to bottom
        </button>
      )}
    </div>
  );
}
