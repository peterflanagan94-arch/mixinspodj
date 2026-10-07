/**
 * MixInspo DJ — AI Suggestions proxy.
 *
 * Holds the real Anthropic API key server-side and forwards requests to it,
 * so a beta tester never needs their own key, and the key is never present
 * in the distributed app where it could be extracted.
 *
 * This is a dumb, stateless pass-through: MixInspo's own Rust code builds
 * the exact request body it would send directly to Anthropic, and this
 * worker just injects the real key and forwards it. All prompt logic stays
 * in the app, not duplicated here.
 *
 * The real safety net against runaway cost is NOT here — it's the spending
 * limit set on the Anthropic key itself, in the Anthropic Console. This
 * worker adds a coarse secondary cap (requests per day) only as a second
 * line of defence.
 */

const ANTHROPIC_URL = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION = "2023-06-01";
/** A coarse daily ceiling, independent of the Anthropic-side spending cap. */
const MAX_REQUESTS_PER_DAY = 500;

export default {
  async fetch(request, env) {
    if (request.method !== "POST") {
      return new Response("Method not allowed", { status: 405 });
    }

    const today = new Date().toISOString().slice(0, 10);
    const key = `count:${today}`;
    const count = Number((await env.AI_PROXY_KV.get(key)) ?? "0");
    if (count >= MAX_REQUESTS_PER_DAY) {
      return new Response(
        JSON.stringify({ error: "Daily request limit reached. Try again tomorrow." }),
        { status: 429, headers: { "content-type": "application/json" } },
      );
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

    // Count every attempt, not just successes, so a buggy or hostile client
    // retrying in a loop still hits the ceiling.
    await env.AI_PROXY_KV.put(key, String(count + 1), { expirationTtl: 60 * 60 * 24 * 2 });

    return new Response(upstream.body, {
      status: upstream.status,
      headers: { "content-type": "application/json" },
    });
  },
};
