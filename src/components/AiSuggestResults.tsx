import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AiSuggestion, AiSuggestionResult } from "../types";
import { SuggestionRow } from "./SuggestionRow";

type Props = {
  nowPlayingId: number | null;
};

const CATEGORIES = [
  { key: "similar_bpm", label: "Similar BPM" },
  { key: "similar_genre", label: "Similar Genre" },
  { key: "popular", label: "Popular Mixes" },
  { key: "wildcard", label: "Wildcard" },
] as const;
type CategoryKey = (typeof CATEGORIES)[number]["key"];

/**
 * AI Suggestions — what Claude knows about the track playing now (genre,
 * era, mood) picking from a real sample of the library, the kind of
 * judgment BPM and key alone can't make. Four angles to ask it from:
 * Similar BPM and Similar Genre each lean on one signal, Popular Mixes
 * leans on the AI's own knowledge of what's broadly popular, and Wildcard
 * ignores fit entirely for a guaranteed floor-filler.
 *
 * Each category is cached locally per song, separately from the others, so
 * switching between them after the first ask of each is instant and free.
 * "Ask again" forces a fresh one for whichever category is showing.
 */
export function AiSuggestResults({ nowPlayingId }: Props) {
  const [hasKey, setHasKey] = useState<boolean | null>(null);
  const [category, setCategory] = useState<CategoryKey | null>(null);
  const [loading, setLoading] = useState(false);
  const [suggestions, setSuggestions] = useState<AiSuggestion[]>([]);
  const [fromCache, setFromCache] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<boolean>("ai_suggestions_available")
      .then(setHasKey)
      .catch(() => setHasKey(false));
  }, []);

  // A new now-playing track invalidates whatever was showing before.
  useEffect(() => {
    setCategory(null);
    setSuggestions([]);
    setError(null);
  }, [nowPlayingId]);

  async function ask(key: CategoryKey, force: boolean) {
    if (nowPlayingId === null) return;
    setCategory(key);
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<AiSuggestionResult>("ai_suggestions", {
        nowPlayingId,
        category: key,
        force,
      });
      setSuggestions(result.suggestions);
      setFromCache(result.from_cache);
    } catch (e) {
      setError(String(e));
      setSuggestions([]);
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
        AI Suggestions isn't set up yet. Add your own Anthropic API key in Settings → AI
        Suggestions, or ask whoever runs MixInspo to finish the shared setup.
      </div>
    );
  }

  return (
    <div className="next-list">
      <div className="ai-category-row">
        {CATEGORIES.map((c) => (
          <button
            key={c.key}
            type="button"
            className={`ai-category-chip ${category === c.key ? "ai-category-chip-active" : ""}`}
            disabled={loading}
            onClick={() => void ask(c.key, false)}
          >
            {c.label}
          </button>
        ))}
      </div>

      {category === null && <div className="next-empty">Pick one above to ask the AI.</div>}

      {category !== null && loading && <p className="hint ai-ask">Usually takes 5–10 seconds, or instant if asked before.</p>}
      {error && <p className="error ai-ask">{error}</p>}

      {category !== null && !loading && suggestions.length > 0 && (
        <>
          <div className="ai-cache-note">
            <span>{fromCache ? "From an earlier request for this song" : "Fresh from the AI"}</span>
            <button type="button" className="ai-ask-again" disabled={loading} onClick={() => void ask(category, true)}>
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
