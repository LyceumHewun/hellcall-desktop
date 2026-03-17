export interface RecognizerConfig {
  chunk_time: number;
  vad_silence_duration: number;
}

export interface KeyPresserConfig {
  wait_open_time: number;
  key_release_interval: number;
  diff_key_interval: number;
}

export interface TriggerConfig {
  hit_word: string | null;
  hit_word_grammar: string | null;
}

export interface CommandConfig {
  _frontendId?: string;
  command: string;
  grammar: string | null;
  shortcut: string | null;
  keys: string[];
  audio_files: string[];
}

export interface AppConfig {
  recognizer: RecognizerConfig;
  key_presser: KeyPresserConfig;
  key_map: Record<string, string>;
  trigger: TriggerConfig;
  commands: CommandConfig[];
}
