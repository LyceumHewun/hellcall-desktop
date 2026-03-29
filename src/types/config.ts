export type TalkMode = "push_to_talk" | "voice_activation";

export interface RecognizerConfig {
  chunk_time: number;
  vad_silence_duration: number;
  talk_mode: TalkMode;
}

export interface MicrophoneConfig {
  enable_denoise: boolean;
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

export interface VisionConfig {
  enable_occ: boolean;
  capture_ratio: number;
}

export interface SpeakerConfig {
  volume: number;
  speed: number;
  sleep_until_end: boolean;
  monitor_local_playback: boolean;
  virtual_mic_enabled: boolean;
  virtual_mic_device: string | null;
  virtual_mic_macro_volume: number;
  virtual_mic_input_volume: number;
}

export interface AppConfig {
  vision: VisionConfig;
  microphone: MicrophoneConfig;
  speaker: SpeakerConfig;
  recognizer: RecognizerConfig;
  key_presser: KeyPresserConfig;
  key_map: Record<string, string>;
  trigger: TriggerConfig;
  commands: CommandConfig[];
}
