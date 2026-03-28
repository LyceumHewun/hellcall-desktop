import { AppConfig } from "../types/config";

export interface EngineStartSnapshot {
  config: AppConfig;
  selectedDevice: string | null;
  selectedVoskModelId: string;
  selectedVisionModelId: string;
}

export function sanitizeConfigForEngine(config: AppConfig): AppConfig {
  const sanitizedConfig = JSON.parse(JSON.stringify(config)) as AppConfig;

  sanitizedConfig.commands = sanitizedConfig.commands
    .filter((cmd) => cmd.command.trim() !== "" && cmd.keys.length > 0)
    .map((cmd) => {
      if (cmd.grammar && cmd.grammar.trim() === "") {
        cmd.grammar = null;
      }

      delete cmd._frontendId;
      return cmd;
    });

  return sanitizedConfig;
}

function normalizeForSignature(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(normalizeForSignature);
  }

  if (value && typeof value === "object") {
    return Object.keys(value as Record<string, unknown>)
      .sort()
      .reduce<Record<string, unknown>>((acc, key) => {
        acc[key] = normalizeForSignature(
          (value as Record<string, unknown>)[key],
        );
        return acc;
      }, {});
  }

  return value;
}

export function createEngineStartSignature(
  snapshot: EngineStartSnapshot,
): string {
  return JSON.stringify(normalizeForSignature(snapshot));
}

export function buildEngineStartSnapshot(
  config: AppConfig,
  selectedDevice: string | null,
  selectedVoskModelId: string,
  selectedVisionModelId: string,
): EngineStartSnapshot {
  return {
    config: sanitizeConfigForEngine(config),
    selectedDevice,
    selectedVoskModelId,
    selectedVisionModelId,
  };
}
