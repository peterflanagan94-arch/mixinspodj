import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AiSuggestion } from "../types";
import { SuggestionRow } from "./SuggestionRow";

type Props = {
  nowPlayingId: number | null;
};

/**
 * AI Suggestions ("AI Suggest") — what Claude knows about the track playing
 * now (genre, era, mood) picking from a real sample of the library, the kind
 * of judgment BPM and key alone can't make. Only runs when asked for: never
 * automatically, and never again for the same track without asking again.
 */
export function AiSuggestResults({ nowPlayingId }: Props) {
  const [hasKey, setHasKey] = useState<boolean | null>(null);
  const [loading, setLoading] = useState(false);
  const [suggestions, setSuggestions] = useState<AiSuggestion[]>([]);
  const [forId, setForId] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<boolean>("has_ai_api_key")
      .then(setHasKey)
      .catch(() => setHasKey(false));
  }, []);

  // A new now-playing track invalidates whatever was asked for before.
  useEffect(() => {
    setSuggestions([]);
    setError(null);
  }, [nowPlayingId]);

  async function ask() {
    if (nowPlayingId === null) return;
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<AiSuggestion[]>("ai_suggestions", { nowPlayingId });
      setSuggestions(result);
      setForId(nowPlayingId);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  if (nowPlayingId === null) {
    return <div className="next-empty">Select a Now Playing track to ask the AI.</div>;
  }
  if (hasKey === false) {
    return (
      <div className="next-empty">
        Add your Anthropic API key in Settings → AI Suggestions to use this.
      </div>
    );
  }

  const showing = forId === nowPlayingId && suggestions.length > 0;

  return (
    <div className="next-list">
      {!showing && (
        <div className="ai-ask">
          <button type="button" className="export-btn" disabled={loading} onClick={() => void ask()}>
            {loading ? "Asking…" : "Get AI suggestions"}
          </button>
          {loading && <p className="hint">Usually takes 5–10 seconds.</p>}
          {error && <p className="error">{error}</p>}
        </div>
      )}
      {showing &&
        suggestions.map((s) => (
          <SuggestionRow
            key={s.track_id}
            title={s.title}
            artist={s.artist}
            bpm={s.bpm}
            camelotKey={s.camelot_key}
            score={s.fit}
            reason={s.reason}
          />
        ))}
    </div>
  );
}
