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

/** Drag-and-drop payload key for "this track" — used to drag a row from
 * the NEXT list (or the Library window) onto the NOW PLAYING card. Native
 * HTML5 DnD, so it works across the two separate Tauri windows too, since
 * the OS — not React — owns the drag session. */
export const TRACK_DRAG_MIME = "application/x-mixinspodj-track-id";

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
  cloud_only: number;
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

export type ScoreComponent = {
  name: string;
  weight: number;
  value: number;
  explanation: string;
};

export type ScoredTrack = {
  track_id: number;
  score_percent: number;
  /** Sorted strongest-contribution-first by the backend. */
  components: ScoreComponent[];
  /** "PARTIAL_DATA" (some components missing), "UNSCORED" (none at all),
   * or "ALREADY_PLAYED" (played earlier this session — penalised, not hidden) */
  badges: string[];
  /** Local time of the last play this session, "YYYY-MM-DD HH:MM:SS". */
  played_at: string | null;
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

export type TraktorTrack = {
  artist: string | null;
  title: string | null;
  track_id: number | null;
  searching: boolean;
};

export type TraktorStatus = {
  state: "listening" | "connected" | "port-busy";
  address: string;
  port: number;
};
