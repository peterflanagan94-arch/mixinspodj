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

export type ImportSummary = {
  entries: number;
  tempo_applied: number;
  indexed: number;
  missing_files: number;
};

export type TopPlayed = {
  track_id: number;
  title: string | null;
  artist: string | null;
  bpm: number | null;
  camelot_key: string | null;
  plays: number;
};

export type SetSummary = {
  session_id: number;
  started_at: string;
  last_play_at: string | null;
  plays: number;
};

export type SetPlay = {
  position: number;
  track_id: number;
  title: string | null;
  artist: string | null;
  bpm: number | null;
  camelot_key: string | null;
  played_at: string;
  source: string;
  mode: string;
  est_seconds: number | null;
};

export type TraktorSet = {
  set_id: number;
  file_name: string;
  plays: number;
};

export type TraktorPlay = {
  title: string;
  artist: string | null;
  track_id: number | null;
  start_seconds: number;
  played_seconds: number;
  deck: number;
};

export type HistoryImportSummary = {
  files: number;
  imported: number;
  unchanged: number;
  plays: number;
};

export type AiSuggestion = {
  track_id: number;
  title: string;
  artist: string | null;
  bpm: number | null;
  camelot_key: string | null;
  fit: number;
  reason: string;
};

export type AiSuggestionResult = {
  suggestions: AiSuggestion[];
  from_cache: boolean;
};

export const SET_ARCS = [
  "warm_up_peak_wind_down",
  "steady_build",
  "peak_time",
  "dinner_then_dancefloor",
] as const;
export type SetArc = (typeof SET_ARCS)[number];

export const SET_ARC_LABELS: Record<SetArc, string> = {
  warm_up_peak_wind_down: "Warm up → Peak → Wind down",
  steady_build: "Steady build",
  peak_time: "Peak time",
  dinner_then_dancefloor: "Dinner, then dancefloor",
};

/** A sensible starting arc for each event type — still just a starting
 * point, the DJ can change it on the panel before building. */
export const DEFAULT_ARC_FOR_EVENT: Record<EventContext, SetArc> = {
  WEDDING: "dinner_then_dancefloor",
  "18TH": "warm_up_peak_wind_down",
  "21ST": "warm_up_peak_wind_down",
  "30TH": "warm_up_peak_wind_down",
  "40TH": "warm_up_peak_wind_down",
  "50TH": "warm_up_peak_wind_down",
  CORPORATE: "warm_up_peak_wind_down",
  CLUB: "peak_time",
  BAR: "peak_time",
  OTHER: "steady_build",
};

export type SetBuilderTrack = {
  track_id: number;
  title: string | null;
  artist: string | null;
  bpm: number | null;
  camelot_key: string | null;
  energy: number | null;
  duration_ms: number | null;
  target_energy: number;
  transition: string;
};

export type SetBuilderResult = {
  tracks: SetBuilderTrack[];
  total_duration_ms: number;
  target_duration_ms: number;
};

export const STARTUP_VIEWS = ["live", "set_prepare"] as const;
export type StartupView = (typeof STARTUP_VIEWS)[number];

export const STARTUP_VIEW_LABELS: Record<StartupView, string> = {
  live: "Live Listen (Now Playing)",
  set_prepare: "Set Prepare (Set Builder)",
};

export type SpotifyTrack = {
  title: string;
  artist: string;
};

export type MatchedTrack = {
  track_id: number;
  spotify_title: string;
  spotify_artist: string;
  library_title: string;
  library_artist: string | null;
};

export type SpotifyImportSummary = {
  playlist_name: string;
  matched: MatchedTrack[];
  unmatched: SpotifyTrack[];
};
