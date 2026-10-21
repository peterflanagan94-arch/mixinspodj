import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

export type AccountState = {
  signed_in: boolean;
  email: string | null;
  allowed: boolean;
  reason: string;
  trial_days_left: number | null;
  must_change_password: boolean;
  subscribe_url: string | null;
  price: string;
  offline: boolean;
};

const SUPPORT = "support@studioonscreen.com";

/**
 * Sits in front of the whole app: sign in or create an account (14-day free
 * trial, no card), choose your own password after the emailed temporary one,
 * and subscribe when the trial ends. Once the account is allowed it renders
 * the app, with a slim banner while the trial runs.
 */
export function AccountGate({ children }: { children: React.ReactNode }) {
  const [state, setState] = useState<AccountState | null>(null);
  const refresh = useCallback(async () => {
    setState(await invoke<AccountState>("account_state"));
  }, []);

  useEffect(() => {
    void refresh();
    // Re-check once an hour so a trial ending or a subscription starting is picked up while the app stays open.
    const id = window.setInterval(() => void refresh(), 60 * 60 * 1000);
    return () => window.clearInterval(id);
  }, [refresh]);

  if (!state) return <main className="app gate"><p className="hint">Loading…</p></main>;
  if (!state.signed_in) return <SignIn onDone={setState} />;
  if (state.must_change_password) return <ChoosePassword onDone={setState} />;
  if (!state.allowed) return <Subscribe state={state} onRecheck={refresh} onSignOut={async () => { await invoke("account_logout"); await refresh(); }} />;

  return (
    <>
      {state.reason === "trial" && state.trial_days_left !== null && (
        <div className="gate-banner">
          Free trial: {state.trial_days_left} {state.trial_days_left === 1 ? "day" : "days"} left.{" "}
          {state.subscribe_url && (
            <a href="#" onClick={(e) => { e.preventDefault(); void openUrl(state.subscribe_url!); }}>
              Subscribe for {state.price}
            </a>
          )}
        </div>
      )}
      {children}
    </>
  );
}

function SignIn({ onDone }: { onDone: (s: AccountState) => void }) {
  const [mode, setMode] = useState<"signin" | "create">("signin");
  const [email, setEmail] = useState("");
  const [name, setName] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      if (mode === "create") {
        await invoke("account_signup", { email, name: name || null });
        setMode("signin");
        setPassword("");
        setMessage(
          "Check your email for a temporary password, then sign in below. " +
            "If you already have a Studio On Screen account, use your existing password instead.",
        );
      } else {
        onDone(await invoke<AccountState>("account_login", { email, password }));
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="app gate">
      <h2>MixInspo DJ</h2>
      <p className="hint">
        {mode === "create"
          ? "Create an account to start your 14-day free trial. No card needed."
          : "Sign in to MixInspo DJ."}
      </p>
      <form onSubmit={submit} className="gate-form">
        <input className="panel-search-input" type="email" placeholder="Email" value={email} onChange={(e) => setEmail(e.target.value)} required />
        {mode === "create" ? (
          <input className="panel-search-input" placeholder="Name (optional)" value={name} onChange={(e) => setName(e.target.value)} />
        ) : (
          <input className="panel-search-input" type="password" placeholder="Password" value={password} onChange={(e) => setPassword(e.target.value)} required />
        )}
        <button type="submit" disabled={busy}>
          {busy ? "One moment…" : mode === "create" ? "Start free trial" : "Sign in"}
        </button>
      </form>
      {message && <p className="hint">{message}</p>}
      {error && <p className="hint gate-error">{error}</p>}
      <p className="hint">
        <a href="#" onClick={(e) => { e.preventDefault(); setMode(mode === "create" ? "signin" : "create"); setError(null); }}>
          {mode === "create" ? "I already have an account" : "New here? Start a free trial"}
        </a>
      </p>
    </main>
  );
}

function ChoosePassword({ onDone }: { onDone: (s: AccountState) => void }) {
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      onDone(await invoke<AccountState>("account_change_password", { password }));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="app gate">
      <h2>Choose your password</h2>
      <p className="hint">Pick your own password to replace the temporary one from your email (8 characters or more).</p>
      <form onSubmit={submit} className="gate-form">
        <input className="panel-search-input" type="password" placeholder="New password" value={password} onChange={(e) => setPassword(e.target.value)} minLength={8} required />
        <button type="submit" disabled={busy}>{busy ? "Saving…" : "Save password"}</button>
      </form>
      {error && <p className="hint gate-error">{error}</p>}
    </main>
  );
}

function Subscribe({ state, onRecheck, onSignOut }: { state: AccountState; onRecheck: () => Promise<void>; onSignOut: () => Promise<void> }) {
  const offlineExpired = state.reason === "offline-expired";
  return (
    <main className="app gate">
      <h2>{offlineExpired ? "Please go online" : "Your free trial has ended"}</h2>
      <p className="hint">
        {offlineExpired
          ? "MixInspo DJ needs to check your account online every so often. Connect to the internet and try again."
          : `Subscribe for ${state.price} to keep using MixInspo DJ. Use ${state.email ?? "the same email"} at checkout so we can match it to your account.`}
      </p>
      {!offlineExpired && state.subscribe_url && (
        <button onClick={() => void openUrl(state.subscribe_url!)}>Subscribe — {state.price}</button>
      )}
      {!offlineExpired && !state.subscribe_url && <p className="hint">Subscriptions are opening soon. Email {SUPPORT}.</p>}
      <button onClick={() => void onRecheck()}>I've subscribed, check again</button>
      <p className="hint">
        Signed in as {state.email}. <a href="#" onClick={(e) => { e.preventDefault(); void onSignOut(); }}>Sign out</a>
      </p>
    </main>
  );
}
