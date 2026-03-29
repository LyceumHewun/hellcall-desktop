export type StratagemDirection = "UP" | "DOWN" | "LEFT" | "RIGHT";

export interface StratagemItem {
  id: string;
  section: string;
  category: string;
  name: string;
  icon_url: string;
  command: StratagemDirection[];
}

export interface StratagemCatalog {
  updated_at_unix: number | null;
  source_url: string;
  items: StratagemItem[];
}
