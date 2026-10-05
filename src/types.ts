export type TrackSummary = {
  id: number;
  title: string | null;
  artist: string | null;
  album: string | null;
  genre: string | null;
  year: number | null;
  duration_ms: number | null;
  bpm: number | null;
  camelot_key: string | null;
  energy: number | null;
  danceability: number | null;
  file_format: string | null;
  file_path: string;
  missing_file: boolean;
};

export type TrackMetadataInput = {
  track_id: number;
  title: string | null;
  artist: string | null;
  album: string | null;
  genre: string | null;
  year: number | null;
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
