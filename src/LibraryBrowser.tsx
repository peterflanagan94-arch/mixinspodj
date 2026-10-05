import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TrackMetadataInput, TrackSummary, TRACK_DRAG_MIME } from "./types";
import "./LibraryBrowser.css";

type SortField =
  | "title"
  | "artist"
  | "album"
  | "genre"
  | "year"
  | "bpm"
  | "camelot_key"
  | "energy"
  | "duration_ms"
  | "file_format";

type SortDir = "asc" | "desc";

const COLUMNS: { field: SortField; label: string; width: string }[] = [
  { field: "title", label: "Title", width: "22%" },
  { field: "artist", label: "Artist", width: "16%" },
  { field: "album", label: "Album", width: "14%" },
  { field: "genre", label: "Genre", width: "10%" },
  { field: "year", label: "Year", width: "6%" },
  { field: "bpm", label: "BPM", width: "6%" },
  { field: "camelot_key", label: "Key", width: "6%" },
  { field: "energy", label: "Energy", width: "6%" },
  { field: "duration_ms", label: "Duration", width: "7%" },
  { field: "file_format", label: "Format", width: "7%" },
];

function formatDuration(ms: number | null): string {
  if (ms === null) return "—";
  const totalSeconds = Math.round(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

function compareValues(a: unknown, b: unknown): number {
  if (a === null && b === null) return 0;
  if (a === null) return 1;
  if (b === null) return -1;
  if (typeof a === "number" && typeof b === "number") return a - b;
  return String(a).localeCompare(String(b));
}

export default function LibraryBrowser() {
  const [tracks, setTracks] = useState<TrackSummary[]>([]);
  const [query, setQuery] = useState("");
  const [sortField, setSortField] = useState<SortField>("artist");
  const [sortDir, setSortDir] = useState<SortDir>("asc");
  const [editing, setEditing] = useState<TrackSummary | null>(null);
  const [loading, setLoading] = useState(true);

  async function refresh() {
    setLoading(true);
    try {
      const result = await invoke<TrackSummary[]>("list_tracks");
      setTracks(result);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    refresh();
  }, []);

  function toggleSort(field: SortField) {
    if (field === sortField) {
      setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    } else {
      setSortField(field);
      setSortDir("asc");
    }
  }

  const visibleTracks = useMemo(() => {
    const q = query.trim().toLowerCase();
    const filtered = q
      ? tracks.filter((t) =>
          [t.title, t.artist, t.album, t.genre].some((field) => field?.toLowerCase().includes(q)),
        )
      : tracks;

    const sorted = [...filtered].sort((a, b) => {
      const result = compareValues(a[sortField], b[sortField]);
      return sortDir === "asc" ? result : -result;
    });
    return sorted;
  }, [tracks, query, sortField, sortDir]);

  return (
    <main className="library">
      <header className="library-header">
        <h1>Library</h1>
        <input
          className="library-search"
          placeholder="Filter by title, artist, album, genre…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <span className="library-count">
          {loading ? "loading…" : `${visibleTracks.length} / ${tracks.length} tracks`}
        </span>
      </header>

      <div className="library-table-wrap">
        <table className="library-table">
          <thead>
            <tr>
              {COLUMNS.map((col) => (
                <th key={col.field} style={{ width: col.width }} onClick={() => toggleSort(col.field)}>
                  {col.label}
                  {sortField === col.field && <span className="sort-arrow">{sortDir === "asc" ? " ▲" : " ▼"}</span>}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {visibleTracks.map((track) => (
              <tr
                key={track.id}
                className={track.missing_file ? "row-missing" : ""}
                onClick={() => setEditing(track)}
                title={track.missing_file ? "File not found at last scan" : undefined}
                draggable
                onDragStart={(e) => {
                  e.dataTransfer.setData(TRACK_DRAG_MIME, String(track.id));
                  e.dataTransfer.effectAllowed = "copy";
                }}
              >
                <td>{track.title ?? "—"}</td>
                <td>{track.artist ?? "—"}</td>
                <td>{track.album ?? "—"}</td>
                <td>{track.genre ?? "—"}</td>
                <td>{track.year ?? "—"}</td>
                <td>{track.bpm !== null ? Math.round(track.bpm) : "—"}</td>
                <td>{track.camelot_key ?? "—"}</td>
                <td>{track.energy !== null ? track.energy.toFixed(1) : "—"}</td>
                <td>{formatDuration(track.duration_ms)}</td>
                <td>{track.file_format ?? "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
        {!loading && visibleTracks.length === 0 && (
          <div className="library-empty">
            {tracks.length === 0 ? "No tracks yet — scan a folder from Settings." : "No matches."}
          </div>
        )}
      </div>

      {editing && (
        <EditTrackPanel
          track={editing}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            refresh();
          }}
        />
      )}
    </main>
  );
}

function EditTrackPanel({
  track,
  onClose,
  onSaved,
}: {
  track: TrackSummary;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [title, setTitle] = useState(track.title ?? "");
  const [artist, setArtist] = useState(track.artist ?? "");
  const [album, setAlbum] = useState(track.album ?? "");
  const [genre, setGenre] = useState(track.genre ?? "");
  const [year, setYear] = useState(track.year?.toString() ?? "");
  const [saving, setSaving] = useState(false);

  async function save() {
    setSaving(true);
    const input: TrackMetadataInput = {
      track_id: track.id,
      title: title.trim() || null,
      artist: artist.trim() || null,
      album: album.trim() || null,
      genre: genre.trim() || null,
      year: year.trim() ? Number(year.trim()) : null,
    };
    try {
      await invoke("update_track_metadata", { edit: input });
      onSaved();
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="panel-overlay" onClick={onClose}>
      <div className="edit-panel" onClick={(e) => e.stopPropagation()}>
        <div className="panel-header">
          <span>EDIT TRACK</span>
          <button className="icon-btn" onClick={onClose}>
            ✕
          </button>
        </div>
        <div className="edit-form">
          <label>
            Title
            <input value={title} onChange={(e) => setTitle(e.target.value)} />
          </label>
          <label>
            Artist
            <input value={artist} onChange={(e) => setArtist(e.target.value)} />
          </label>
          <label>
            Album
            <input value={album} onChange={(e) => setAlbum(e.target.value)} />
          </label>
          <label>
            Genre
            <input value={genre} onChange={(e) => setGenre(e.target.value)} />
          </label>
          <label>
            Year
            <input value={year} onChange={(e) => setYear(e.target.value)} inputMode="numeric" />
          </label>
          <p className="edit-hint">{track.file_path}</p>
          <div className="edit-actions">
            <button onClick={onClose} disabled={saving}>
              Cancel
            </button>
            <button className="primary" onClick={save} disabled={saving}>
              {saving ? "Saving…" : "Save"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
