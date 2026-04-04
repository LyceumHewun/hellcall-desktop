export type TalkMode = "push_to_talk" | "voice_activation";
export type AppMode = "voice_command" | "ai_agent";

export interface AiAgentConfig {
  id: string;
  name: string;
  description: string;
  system_prompt: string;
  chat_model: string;
  temperature: number;
  max_tokens: number;
  enable_thinking: boolean;
  skill_ids: string[];
  is_builtin: boolean;
}

export interface AiConfig {
  provider: string;
  base_url: string;
  api_key: string;
  default_chat_model: string;
  default_asr_model: string;
  tts_enabled: boolean;
  default_tts_model: string;
  auto_execute_skills: boolean;
  default_agent_id: string;
  agents: AiAgentConfig[];
}

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
  mode: AppMode;
  ai: AiConfig;
  vision: VisionConfig;
  microphone: MicrophoneConfig;
  speaker: SpeakerConfig;
  recognizer: RecognizerConfig;
  key_presser: KeyPresserConfig;
  key_map: Record<string, string>;
  trigger: TriggerConfig;
  commands: CommandConfig[];
}
