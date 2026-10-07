# MixInspo AI Suggestions proxy

A tiny Cloudflare Worker that holds the real Anthropic API key so beta
testers never need their own. Deploy it yourself — this needs your own
Cloudflare login, which Claude can't do for you.

## Deploy (run these yourself, in your own terminal)

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

```bash
npx wrangler deploy
```

This prints a URL like `https://mixinspo-ai-proxy.<your-subdomain>.workers.dev`.
That URL is **not** secret — tell Claude what it printed, so it can be set
as the app's default proxy address.

## Also do this (the real safety net)

In the Anthropic Console, on this API key (or its workspace), set a monthly
spending limit. The worker's own daily request cap (`MAX_REQUESTS_PER_DAY`
in `worker.js`) is a second line of defence, not the main one.
