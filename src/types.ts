export type TrackSummary = {
  id: number;
  title: string | null;
  artist: string | null;
  bpm: number | null;
  camelot_key: string | null;
  energy: number | null;
  file_path: string;
};

export type ScanProgress = {
  current: number;
  total: number;
  current_file: string;
};

export type ScanSummary = {
  folder: string;
  files_found: number;
  inserted: number;
  updated: number;
  duplicates_found: number;
  errors: number;
};

export type AnalyzeProgress = {
  current: number;
  total: number;
  current_file: string;
};

export type AnalyzeSummary = {
  total: number;
  analyzed: number;
  errors: number;
};

export const DIRECTION_MODES = ["KEEP", "BUILD", "COOL", "SWITCH", "RESCUE"] as const;
export type DirectionMode = (typeof DIRECTION_MODES)[number];

export const EVENT_CONTEXTS = [
  "WEDDING",
  "18TH",
  "21ST",
  "30TH",
  "40TH",
  "50TH",
  "CORPORATE",
  "CLUB",
  "BAR",
  "OTHER",
] as const;
export type EventContext = (typeof EVENT_CONTEXTS)[number];
