/**
 * MixInspo DJ — shared backend.
 *
 * Two independent jobs behind one Worker:
 *
 * 1. AI Suggestions proxy (POST /). Holds the real Anthropic API key
 *    server-side and forwards requests to it, so a beta tester never needs
 *    their own key, and the key is never present in the distributed app
 *    where it could be extracted. A dumb, stateless pass-through: MixInspo's
 *    own Rust code builds the exact request body it would send directly to
 *    Anthropic, and this worker just injects the real key and forwards it.
 *    All prompt logic stays in the app, not duplicated here.
 *
 *    The real safety net against runaway cost is NOT here — it's the
 *    spending limit set on the Anthropic key itself, in the Anthropic
 *    Console. This worker adds a coarse secondary cap (requests per day)
 *    only as a second line of defence.
 *
 * 2. Anonymous play log (POST /plays, GET /popular). Each DJ who hasn't
 *    turned this off in Settings sends the track, artist, BPM, key and
 *    genre of what they play — nothing that identifies them — so a real
 *    "popular with MixInspo DJs" list can be built from actual usage.
 */

const ANTHROPIC_URL = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION = "2023-06-01";
/** A coarse daily ceiling, independent of the Anthropic-side spending cap. */
const MAX_AI_REQUESTS_PER_DAY = 500;
/** A generous daily ceiling on play reports — bounds abuse, not real usage. */
const MAX_PLAYS_PER_DAY = 5000;
/** Plain sanity limits on play fields, so one bad client can't write junk. */
const MAX_FIELD_LENGTH = 200;

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    if (url.pathname === "/plays" && request.method === "POST") {
      return handlePlay(request, env);
    }
    if (url.pathname === "/popular" && request.method === "GET") {
      return handlePopular(url, env);
    }
    if (url.pathname === "/" || url.pathname === "") {
      return handleAiProxy(request, env);
    }
    return new Response("Not found", { status: 404 });
  },
};

async function handleAiProxy(request, env) {
  if (request.method !== "POST") {
    return new Response("Method not allowed", { status: 405 });
  }

  const allowed = await withinDailyLimit(env, "ai", MAX_AI_REQUESTS_PER_DAY);
  if (!allowed) {
    return jsonResponse({ error: "Daily request limit reached. Try again tomorrow." }, 429);
  }

  const body = await request.text();
  const upstream = await fetch(ANTHROPIC_URL, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-api-key": env.ANTHROPIC_API_KEY,
      "anthropic-version": ANTHROPIC_VERSION,
    },
    body,
  });

  return new Response(upstream.body, {
    status: upstream.status,
    headers: { "content-type": "application/json" },
  });
}

async function handlePlay(request, env) {
  const allowed = await withinDailyLimit(env, "plays", MAX_PLAYS_PER_DAY);
  if (!allowed) {
    return jsonResponse({ error: "Daily limit reached." }, 429);
  }

  let data;
  try {
    data = await request.json();
  } catch {
    return jsonResponse({ error: "Invalid JSON body." }, 400);
  }

  const title = clean(data.title);
  if (!title) {
    return jsonResponse({ error: "title is required." }, 400);
  }
  const artist = clean(data.artist);
  const key = clean(data.key);
  const genre = clean(data.genre);
  const bpm = typeof data.bpm === "number" && Number.isFinite(data.bpm) ? data.bpm : null;

  await env.PLAYS_DB.prepare(
    "INSERT INTO plays (title, artist, bpm, camelot_key, genre) VALUES (?1, ?2, ?3, ?4, ?5)",
  )
    .bind(title, artist, bpm, key, genre)
    .run();

  return jsonResponse({ ok: true });
}

async function handlePopular(url, env) {
  const limit = Math.min(Math.max(Number(url.searchParams.get("limit")) || 100, 1), 500);
  const result = await env.PLAYS_DB.prepare(
    `SELECT title, artist, COUNT(*) as plays
     FROM plays
     GROUP BY lower(trim(title)), lower(trim(COALESCE(artist, '')))
     ORDER BY plays DESC
     LIMIT ?1`,
  )
    .bind(limit)
    .all();

  return jsonResponse({ tracks: result.results });
}

/** A field, trimmed and length-capped, or null for anything empty/invalid. */
function clean(value) {
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  if (!trimmed) return null;
  return trimmed.slice(0, MAX_FIELD_LENGTH);
}

/** True if `name`'s count for today is still under `limit`; also records this attempt. */
async function withinDailyLimit(env, name, limit) {
  const today = new Date().toISOString().slice(0, 10);
  const key = `count:${name}:${today}`;
  const count = Number((await env.AI_PROXY_KV.get(key)) ?? "0");
  if (count >= limit) return false;
  // Counted before the real work runs, so a buggy or hostile client retrying
  // in a loop still hits the ceiling.
  await env.AI_PROXY_KV.put(key, String(count + 1), { expirationTtl: 60 * 60 * 24 * 2 });
  return true;
}

function jsonResponse(body, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });
}
