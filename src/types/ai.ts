export interface AiSessionEvent {
  id: string;
  kind: string;
  text: string | null;
  created_at_ms: number;
}

export interface AiSessionSummary {
  id: string;
  title: string;
  created_at_ms: number;
  updated_at_ms: number;
  message_count: number;
}

export interface AiSessionRecord extends AiSessionSummary {
  events: AiSessionEvent[];
}

export interface AiLiveToolActivity {
  id: string;
  session_id: string;
  phase: "call" | "result" | "error";
  name: string;
  summary: string;
}
