# MixInspo shared backend

One small Cloudflare Worker doing two jobs:

1. **AI Suggestions proxy** — holds the real Anthropic API key so beta
   testers never need their own.
2. **Anonymous play log** — each DJ's plays (unless they've turned this off
   in Settings), used to build a real "popular with MixInspo DJs" list.

This is already deployed for the AI proxy. The steps below add the play log
on top of it. All of this needs your own Cloudflare login, which Claude
can't do for you — run every command yourself, in your own terminal.

## First-time deploy (if you haven't already)

```bash
cd server/ai-proxy
npx wrangler login
npx wrangler kv namespace create AI_PROXY_KV
```

Paste the `id` that prints into `wrangler.toml` in place of
`REPLACE_WITH_KV_ID`, then:

```bash
npx wrangler secret put ANTHROPIC_API_KEY
```

Paste your real Anthropic key when prompted — this goes straight into
Cloudflare's secret store, never into a file or chat.

## Adding the play log

```bash
npx wrangler d1 create mixinspo-plays
```

Paste the `database_id` it prints into `wrangler.toml` in place of
`REPLACE_WITH_D1_ID`, then create the table:

```bash
npx wrangler d1 execute mixinspo-plays --remote --file=schema.sql
```

## Deploy (first time, or after any change above)

```bash
npx wrangler deploy
```

This prints a URL like `https://mixinspo-ai-proxy.<your-subdomain>.workers.dev`.
That URL is **not** secret.

## Checking the play log

```bash
curl "https://mixinspo-ai-proxy.<your-subdomain>.workers.dev/popular?limit=20"
```

Or query the database directly:

```bash
npx wrangler d1 execute mixinspo-plays --remote \
  --command "SELECT title, artist, COUNT(*) as plays FROM plays GROUP BY title, artist ORDER BY plays DESC LIMIT 20"
```

## Also do this (the real safety net)

In the Anthropic Console, on this API key (or its workspace), set a monthly
spending limit. The worker's own daily request cap (`MAX_AI_REQUESTS_PER_DAY`
in `worker.js`) is a second line of defence, not the main one. The play log
has its own separate daily cap (`MAX_PLAYS_PER_DAY`) — generous enough for
real usage, just bounding abuse.
