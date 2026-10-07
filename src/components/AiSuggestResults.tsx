import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AiSuggestion, AiSuggestionResult } from "../types";
import { SuggestionRow } from "./SuggestionRow";

type Props = {
  nowPlayingId: number | null;
};

/**
 * AI Suggestions ("AI Suggest") — what Claude knows about the track playing
 * now (genre, era, mood) picking from a real sample of the library, the kind
 * of judgment BPM and key alone can't make.
 *
 * Results are cached locally by song (title + artist), so asking about the
 * same track again — a different night, a different session — reuses the
 * answer instead of paying for and waiting on another API call. "Ask again"
 * forces a fresh one.
 */
export function AiSuggestResults({ nowPlayingId }: Props) {
  const [hasKey, setHasKey] = useState<boolean | null>(null);
  const [loading, setLoading] = useState(false);
  const [suggestions, setSuggestions] = useState<AiSuggestion[]>([]);
  const [fromCache, setFromCache] = useState(false);
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

  async function ask(force: boolean) {
    if (nowPlayingId === null) return;
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<AiSuggestionResult>("ai_suggestions", { nowPlayingId, force });
      setSuggestions(result.suggestions);
      setFromCache(result.from_cache);
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
          <button type="button" className="export-btn" disabled={loading} onClick={() => void ask(false)}>
            {loading ? "Asking…" : "Get AI suggestions"}
          </button>
          {loading && <p className="hint">Usually takes 5–10 seconds, or instant if asked before.</p>}
          {error && <p className="error">{error}</p>}
        </div>
      )}
      {showing && (
        <>
          <div className="ai-cache-note">
            <span>{fromCache ? "From an earlier request for this song" : "Fresh from the AI"}</span>
            <button type="button" className="ai-ask-again" disabled={loading} onClick={() => void ask(true)}>
              {loading ? "Asking…" : "Ask again"}
            </button>
          </div>
          {suggestions.map((s) => (
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
        </>
      )}
    </div>
  );
}
