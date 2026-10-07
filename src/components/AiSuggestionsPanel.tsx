import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AiSuggestion } from "../types";

type Props = {
  open: boolean;
  onClose: () => void;
  nowPlayingId: number | null;
  onSelectTrack: (trackId: number) => void;
};

/**
 * AI Suggestions: asks Claude what it knows about the track playing now to
 * pick from a real sample of the library — genre, era and mood, which BPM
 * and key alone can't see. Separate from NEXT: this is the one part of the
 * app that calls out to the internet, with the DJ's own API key, so it only
 * runs when asked for, never automatically.
 */
export function AiSuggestionsPanel({ open, onClose, nowPlayingId, onSelectTrack }: Props) {
  const [hasKey, setHasKey] = useState<boolean | null>(null);
  const [keyInput, setKeyInput] = useState("");
  const [savingKey, setSavingKey] = useState(false);
  const [loading, setLoading] = useState(false);
  const [suggestions, setSuggestions] = useState<AiSuggestion[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setError(null);
    setSuggestions([]);
    invoke<boolean>("has_ai_api_key")
      .then(setHasKey)
      .catch(() => setHasKey(false));
  }, [open]);

  async function saveKey() {
    if (!keyInput.trim()) return;
    setSavingKey(true);
    try {
      await invoke("set_ai_api_key", { key: keyInput.trim() });
      setKeyInput("");
      setHasKey(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setSavingKey(false);
    }
  }

  async function getSuggestions() {
    if (nowPlayingId === null) return;
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<AiSuggestion[]>("ai_suggestions", { nowPlayingId });
      setSuggestions(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  if (!open) return null;

  return (
    <div className="panel-overlay" onClick={onClose}>
      <div className="panel" onClick={(e) => e.stopPropagation()}>
        <div className="panel-header">
          <span>AI SUGGESTIONS</span>
          <button className="icon-btn" onClick={onClose}>
            ✕
          </button>
        </div>
        <div className="panel-results">
          {hasKey === false && (
            <>
              <p className="hint">
                Add your own Anthropic API key to use this. It's sent only to Anthropic's
                API, from your computer, when you press "Get suggestions" below — never
                automatically, and nowhere else in the app uses the internet.
              </p>
              <input
                type="password"
                autoComplete="off"
                spellCheck={false}
                className="panel-search-input"
                placeholder="sk-ant-…"
                value={keyInput}
                onChange={(e) => setKeyInput(e.target.value)}
              />
              <button type="button" className="export-btn" disabled={savingKey} onClick={() => void saveKey()}>
                {savingKey ? "Saving…" : "Save key"}
              </button>
            </>
          )}

          {hasKey === true && (
            <>
              <button
                type="button"
                className="export-btn"
                disabled={nowPlayingId === null || loading}
                onClick={() => void getSuggestions()}
              >
                {loading ? "Asking…" : "Get suggestions"}
              </button>
              {loading && <p className="hint">Usually takes 5–10 seconds.</p>}
              {nowPlayingId === null && <p className="hint">Select a Now Playing track first.</p>}
            </>
          )}

          {error && <p className="error">{error}</p>}

          {suggestions.map((s) => (
            <button
              key={s.track_id}
              className="search-result-row"
              onClick={() => {
                onSelectTrack(s.track_id);
                onClose();
              }}
            >
              <div>
                <div className="search-result-title">{s.title}</div>
                <div className="search-result-artist">{s.artist ?? "Unknown artist"}</div>
                <div className="search-result-artist">{s.reason}</div>
              </div>
              <div className="search-result-meta">
                {s.bpm !== null ? Math.round(s.bpm) : "—"} · {s.camelot_key ?? "—"}
              </div>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
