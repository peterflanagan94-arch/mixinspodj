import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TrackSummary } from "../types";

type Props = {
  open: boolean;
  onClose: () => void;
  onSelectTrack: (track: TrackSummary) => void;
};

export function SearchPanel({ open, onClose, onSelectTrack }: Props) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<TrackSummary[]>([]);
  const [searched, setSearched] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!open) {
      setQuery("");
      setResults([]);
      setSearched(false);
      setCopied(false);
    }
  }, [open]);

  useEffect(() => {
    setCopied(false);
    if (query.trim().length === 0) {
      setResults([]);
      setSearched(false);
      return;
    }
    const handle = setTimeout(() => {
      invoke<TrackSummary[]>("search_tracks", { query: query.trim() })
        .then((r) => {
          setResults(r);
          setSearched(true);
        })
        .catch(() => {
          setResults([]);
          setSearched(true);
        });
    }, 150);
    return () => clearTimeout(handle);
  }, [query]);

  if (!open) return null;

  return (
    <div className="panel-overlay" onClick={onClose}>
      <div className="panel" onClick={(e) => e.stopPropagation()}>
        <div className="panel-header">
          <span>FIND A TRACK</span>
          <button className="icon-btn" onClick={onClose}>
            ✕
          </button>
        </div>
        <input
          autoFocus
          spellCheck={false}
          autoCorrect="off"
          autoCapitalize="off"
          className="panel-search-input"
          placeholder="Artist, title…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <div className="panel-results">
          {results.map((track) => (
            <button
              key={track.id}
              className="search-result-row"
              onClick={() => {
                onSelectTrack(track);
                onClose();
              }}
            >
              <div>
                <div className="search-result-title">{track.title ?? "Untitled"}</div>
                <div className="search-result-artist">{track.artist ?? "Unknown artist"}</div>
              </div>
              <span className="pill pill-cyan">✓ IN LIBRARY</span>
            </button>
          ))}
          {searched && results.length === 0 && (
            <div className="search-result-row search-result-missing">
              <div>
                <div className="search-result-title">Not in your library</div>
                <div className="search-result-artist">
                  GET MUSIC isn't built yet — copy the name to search elsewhere for now
                </div>
              </div>
              <button
                className="pill pill-yellow"
                onClick={() => {
                  navigator.clipboard.writeText(query.trim());
                  setCopied(true);
                }}
              >
                {copied ? "COPIED" : "COPY"}
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
