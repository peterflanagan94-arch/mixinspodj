import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import {
  AnalyzeProgress,
  AnalyzeSummary,
  ImportSummary,
  ScanProgress,
  ScanSummary,
  STARTUP_VIEW_LABELS,
  STARTUP_VIEWS,
  StartupView,
  TraktorStatus,
} from "../types";

type Props = {
  open: boolean;
  onClose: () => void;
  onLibraryChanged: () => void;
  traktorStatus: TraktorStatus | null;
};

export function SettingsPanel({ open, onClose, onLibraryChanged, traktorStatus }: Props) {
  const [folder, setFolder] = useState<string | null>(null);
  const [scanProgress, setScanProgress] = useState<ScanProgress | null>(null);
  const [scanSummary, setScanSummary] = useState<ScanSummary | null>(null);
  const [scanning, setScanning] = useState(false);
  const [scanError, setScanError] = useState<string | null>(null);

  const [watchedFolders, setWatchedFolders] = useState<string[]>([]);
  const [traktorCollection, setTraktorCollection] = useState<string | null>(null);
  const [aiKeySet, setAiKeySet] = useState<boolean | null>(null);
  const [aiKeyInput, setAiKeyInput] = useState("");
  const [savingAiKey, setSavingAiKey] = useState(false);
  const [sharePlays, setSharePlays] = useState<boolean | null>(null);
  const [startupView, setStartupView] = useState<StartupView | null>(null);
  const [spotifyCredsSet, setSpotifyCredsSet] = useState<boolean | null>(null);
  const [spotifyClientId, setSpotifyClientId] = useState("");
  const [spotifyClientSecret, setSpotifyClientSecret] = useState("");
  const [savingSpotify, setSavingSpotify] = useState(false);

  const [updateStatus, setUpdateStatus] = useState<string | null>(null);
  const [updating, setUpdating] = useState(false);

  async function checkForUpdate() {
    setUpdating(true);
    setUpdateStatus("Checking…");
    try {
      const update = await check();
      if (!update) {
        setUpdateStatus("You're on the latest version.");
        return;
      }
      setUpdateStatus(`Downloading version ${update.version}…`);
      await update.downloadAndInstall();
      setUpdateStatus("Updated — restarting…");
      await relaunch();
    } catch (e) {
      setUpdateStatus(`Could not check for updates: ${String(e)}`);
    } finally {
      setUpdating(false);
    }
  }

  const [bugMessage, setBugMessage] = useState("");
  const [bugEmail, setBugEmail] = useState("");
  const [sendingBug, setSendingBug] = useState(false);
  const [bugStatus, setBugStatus] = useState<string | null>(null);

  async function sendBugReport() {
    setSendingBug(true);
    setBugStatus(null);
    try {
      await invoke("send_bug_report", { message: bugMessage, email: bugEmail || null });
      setBugMessage("");
      setBugStatus("Thanks — your report was sent.");
    } catch (e) {
      setBugStatus(String(e));
    } finally {
      setSendingBug(false);
    }
  }

  async function toggleSharePlays(enabled: boolean) {
    setSharePlays(enabled);
    try {
      await invoke("set_share_plays", { enabled });
    } catch (e) {
      setScanError(String(e));
    }
  }

  async function changeStartupView(view: StartupView) {
    setStartupView(view);
    try {
      await invoke("set_startup_view", { view });
    } catch (e) {
      setScanError(String(e));
    }
  }

  async function removeAiKey() {
    await invoke("clear_ai_api_key");
    setAiKeySet(false);
  }

  async function removeSpotifyCredentials() {
    await invoke("clear_spotify_credentials");
    setSpotifyCredsSet(false);
  }

  async function saveSpotifyCredentials() {
    if (!spotifyClientId.trim() || !spotifyClientSecret.trim()) return;
    setSavingSpotify(true);
    try {
      await invoke("set_spotify_credentials", {
        clientId: spotifyClientId.trim(),
        clientSecret: spotifyClientSecret.trim(),
      });
      setSpotifyClientId("");
      setSpotifyClientSecret("");
      setSpotifyCredsSet(true);
    } catch (e) {
      setScanError(String(e));
    } finally {
      setSavingSpotify(false);
    }
  }

  async function saveAiKey() {
    if (!aiKeyInput.trim()) return;
    setSavingAiKey(true);
    try {
      await invoke("set_ai_api_key", { key: aiKeyInput.trim() });
      setAiKeyInput("");
      setAiKeySet(true);
    } catch (e) {
      setScanError(String(e));
    } finally {
      setSavingAiKey(false);
    }
  }
  const [traktorImport, setTraktorImport] = useState<ImportSummary | null>(null);
  const [virtualdjDatabase, setVirtualdjDatabase] = useState<string | null>(null);
  const [virtualdjImport, setVirtualdjImport] = useState<ImportSummary | null>(null);
  const [importingVdj, setImportingVdj] = useState(false);

  async function loadVirtualdjDatabase() {
    try {
      setVirtualdjDatabase(await invoke<string | null>("find_virtualdj_database"));
    } catch (e) {
      setScanError(String(e));
    }
  }

  async function chooseVirtualdjDatabase() {
    setScanError(null);
    try {
      const picked = await invoke<string | null>("choose_virtualdj_database");
      if (picked) setVirtualdjDatabase(picked);
    } catch (e) {
      setScanError(String(e));
    }
  }

  async function importVirtualdjDatabase() {
    if (!virtualdjDatabase) return;
    setImportingVdj(true);
    setScanError(null);
    try {
      setVirtualdjImport(await invoke<ImportSummary>("import_virtualdj_database", { path: virtualdjDatabase }));
      onLibraryChanged();
    } catch (e) {
      setScanError(String(e));
    } finally {
      setImportingVdj(false);
    }
  }

  async function loadTraktorCollection() {
    try {
      setTraktorCollection(await invoke<string | null>("active_traktor_collection"));
    } catch (e) {
      setScanError(String(e));
    }
  }

  async function chooseTraktorCollection() {
    setScanError(null);
    try {
      const picked = await invoke<string | null>("choose_traktor_collection");
      if (!picked) return;
      setTraktorCollection(picked);
      setTraktorImport(await invoke<ImportSummary>("import_traktor_collection"));
      onLibraryChanged();
    } catch (e) {
      setScanError(String(e));
    }
  }

  async function loadWatchedFolders() {
    try {
      setWatchedFolders(await invoke<string[]>("list_watched_folders"));
    } catch (e) {
      setScanError(String(e));
    }
  }

  async function removeWatchedFolder(path: string) {
    await invoke("remove_watched_folder", { path });
    await loadWatchedFolders();
    onLibraryChanged();
  }

  useEffect(() => {
    if (open) {
      void loadWatchedFolders();
      void loadTraktorCollection();
      void loadVirtualdjDatabase();
      invoke<boolean>("has_ai_api_key").then(setAiKeySet).catch(() => setAiKeySet(false));
      invoke<boolean>("has_share_plays_enabled").then(setSharePlays).catch(() => setSharePlays(true));
      invoke<StartupView>("get_startup_view").then(setStartupView).catch(() => setStartupView("live"));
      invoke<boolean>("has_spotify_credentials").then(setSpotifyCredsSet).catch(() => setSpotifyCredsSet(false));
    }
  }, [open]);

  const [analyzeProgress, setAnalyzeProgress] = useState<AnalyzeProgress | null>(null);
  const [analyzeSummary, setAnalyzeSummary] = useState<AnalyzeSummary | null>(null);
  const [analyzing, setAnalyzing] = useState(false);
  const [analyzeError, setAnalyzeError] = useState<string | null>(null);

  useEffect(() => {
    const unlistenScan = listen<ScanProgress>("scan-progress", (event) => {
      setScanProgress(event.payload);
    });
    const unlistenAnalyze = listen<AnalyzeProgress>("analysis-progress", (event) => {
      setAnalyzeProgress(event.payload);
    });
    return () => {
      unlistenScan.then((f) => f());
      unlistenAnalyze.then((f) => f());
    };
  }, []);

  async function chooseFolder() {
    setScanError(null);
    const picked = await invoke<string | null>("pick_music_folder");
    if (!picked) return;
    setFolder(picked);
    setScanSummary(null);
    setScanProgress(null);
    setScanning(true);
    try {
      const result = await invoke<ScanSummary>("scan_library_folder", { folder: picked });
      setScanSummary(result);
      onLibraryChanged();
    } catch (e) {
      setScanError(String(e));
    } finally {
      setScanning(false);
    }
  }

  async function analyzeLibrary() {
    setAnalyzeError(null);
    setAnalyzeSummary(null);
    setAnalyzeProgress(null);
    setAnalyzing(true);
    try {
      const result = await invoke<AnalyzeSummary>("analyze_library");
      setAnalyzeSummary(result);
      onLibraryChanged();
    } catch (e) {
      setAnalyzeError(String(e));
    } finally {
      setAnalyzing(false);
    }
  }

  if (!open) return null;

  return (
    <div className="panel-overlay" onClick={onClose}>
      <div className="panel" onClick={(e) => e.stopPropagation()}>
        <div className="panel-header">
          <span>SETTINGS</span>
          <button className="icon-btn" onClick={onClose}>
            ✕
          </button>
        </div>

        <div className="settings-body">
        <div className="settings-section">
          <div className="settings-section-label">Library</div>
          <p className="hint">Select a folder of music files to scan into your library.</p>
          <button onClick={chooseFolder} disabled={scanning}>
            {scanning ? "Scanning…" : "Select Music Folder"}
          </button>

          {folder && <p className="folder-path">{folder}</p>}

          {watchedFolders.length > 0 && (
            <div className="watched-folders">
              <p className="hint">New files in these folders are added automatically:</p>
              {watchedFolders.map((path) => (
                <div className="watched-folder-row" key={path}>
                  <span className="folder-path">{path}</span>
                  <button onClick={() => removeWatchedFolder(path)}>Stop watching</button>
                </div>
              ))}
            </div>
          )}

          {scanning && scanProgress && (
            <div className="progress">
              <div className="progress-bar">
                <div
                  className="progress-bar-fill"
                  style={{
                    width:
                      scanProgress.total > 0
                        ? `${(scanProgress.current / scanProgress.total) * 100}%`
                        : "0%",
                  }}
                />
              </div>
              <p className="progress-label">
                {scanProgress.current} / {scanProgress.total} — {scanProgress.current_file}
              </p>
            </div>
          )}

          {scanError && <p className="error">{scanError}</p>}

          {scanSummary && (
            <div className="summary">
              <div className="summary-row">
                <span>Files found</span>
                <span>{scanSummary.files_found}</span>
              </div>
              <div className="summary-row">
                <span>Added</span>
                <span>{scanSummary.inserted}</span>
              </div>
              <div className="summary-row">
                <span>Updated</span>
                <span>{scanSummary.updated}</span>
              </div>
              <div className="summary-row">
                <span>Possible duplicates</span>
                <span>{scanSummary.duplicates_found}</span>
              </div>
              <div className="summary-row">
                <span>In the cloud only (skipped)</span>
                <span>{scanSummary.cloud_only}</span>
              </div>
              <div className="summary-row">
                <span>Errors</span>
                <span>{scanSummary.errors}</span>
              </div>
            </div>
          )}
        </div>

        <div className="settings-section">
          <div className="settings-section-label">Analysis</div>
          <p className="hint">
            Detect BPM, key, and energy for tracks that haven't been analyzed yet. Runs locally,
            one track at a time, in the background.
          </p>
          <button onClick={analyzeLibrary} disabled={analyzing}>
            {analyzing ? "Analyzing…" : "Analyze Library"}
          </button>

          {analyzing && analyzeProgress && (
            <div className="progress">
              <div className="progress-bar">
                <div
                  className="progress-bar-fill"
                  style={{
                    width:
                      analyzeProgress.total > 0
                        ? `${(analyzeProgress.current / analyzeProgress.total) * 100}%`
                        : "0%",
                  }}
                />
              </div>
              <p className="progress-label">
                {analyzeProgress.current} / {analyzeProgress.total} — {analyzeProgress.current_file}
              </p>
            </div>
          )}

          {analyzeError && <p className="error">{analyzeError}</p>}

          {analyzeSummary && (
            <div className="summary">
              <div className="summary-row">
                <span>Pending</span>
                <span>{analyzeSummary.total}</span>
              </div>
              <div className="summary-row">
                <span>Analyzed</span>
                <span>{analyzeSummary.analyzed}</span>
              </div>
              <div className="summary-row">
                <span>Errors</span>
                <span>{analyzeSummary.errors}</span>
              </div>
            </div>
          )}
        </div>

        <div className="settings-section">
          <div className="settings-section-label">Traktor Pro</div>
          {traktorStatus?.state === "port-busy" ? (
            <p className="error">
              Port {traktorStatus.port} is in use by another app. Quit it and restart MixInspo DJ.
            </p>
          ) : (
            <p className="hint">
              In Traktor: Preferences → Broadcasting → Server Settings. Address{" "}
              <strong>{traktorStatus?.address ?? "127.0.0.1"}</strong>, Port{" "}
              <strong>{traktorStatus?.port ?? 8765}</strong>, Mount path <strong>/mixinspo</strong>,
              Password <strong>mixinspo</strong>. Then turn Broadcasting on.
            </p>
          )}
          <p className="hint">
            Collection:{" "}
            <strong>{traktorCollection ?? "none found"}</strong>
          </p>
          <button type="button" onClick={() => void chooseTraktorCollection()}>
            Choose Traktor collection…
          </button>
          {traktorImport && (
            <p className="hint">
              Imported {traktorImport.tempo_applied} tracks&apos; BPM and key
              {traktorImport.indexed > 0 ? `, added ${traktorImport.indexed} new files` : ""}.
            </p>
          )}
          <p className="hint">
            Status:{" "}
            {traktorStatus?.state === "connected" ? (
              <strong className="traktor-connected">Traktor is connected</strong>
            ) : traktorStatus?.state === "listening"
                ? "waiting for Traktor"
                : "starting…"}
          </p>
        </div>

        <div className="settings-section">
          <div className="settings-section-label">VirtualDJ (experimental)</div>
          <p className="hint">
            BPM and key import from VirtualDJ's own database file — built from community
            documentation, not tested against a real VirtualDJ install. There's no live
            now-playing detection for VirtualDJ yet, only this import, and it only runs when you
            press Import below, never automatically.
          </p>
          <p className="hint">
            Database:{" "}
            <strong>{virtualdjDatabase ?? "none found"}</strong>
          </p>
          <button type="button" onClick={() => void chooseVirtualdjDatabase()}>
            Choose VirtualDJ database…
          </button>
          {virtualdjDatabase && (
            <button type="button" disabled={importingVdj} onClick={() => void importVirtualdjDatabase()}>
              {importingVdj ? "Importing…" : "Import"}
            </button>
          )}
          {virtualdjImport && (
            <p className="hint">
              Found {virtualdjImport.entries} tracks in the database. Applied BPM/key to{" "}
              {virtualdjImport.tempo_applied}
              {virtualdjImport.indexed > 0 ? `, added ${virtualdjImport.indexed} new files` : ""}.{" "}
              {virtualdjImport.missing_files > 0 &&
                `${virtualdjImport.missing_files} entries pointed at files not on this Mac.`}{" "}
              If this looks wrong — wrong BPMs, or far fewer matches than you expected — tell
              Peter; it means the unverified parts of this parser need fixing against your file.
            </p>
          )}
        </div>

        <div className="settings-section">
          <div className="settings-section-label">AI Suggestions</div>
          <p className="hint">
            {aiKeySet === true
              ? "Your own key is saved and used instead of the shared one. AI Suggestions sends the now-playing track and a sample of your library's titles, artists, BPM, key and genre only when you press \"Get AI suggestions\" there."
              : "AI Suggestions works out of the box, through MixInspo's own shared service. Add your own Anthropic API key here only if you'd rather use that instead."}
          </p>
          {aiKeySet === true ? (
            <button type="button" onClick={() => void removeAiKey()}>
              Remove key
            </button>
          ) : (
            <>
              <input
                type="password"
                autoComplete="off"
                spellCheck={false}
                className="panel-search-input"
                placeholder="sk-ant-…"
                value={aiKeyInput}
                onChange={(e) => setAiKeyInput(e.target.value)}
              />
              <button type="button" disabled={savingAiKey} onClick={() => void saveAiKey()}>
                {savingAiKey ? "Saving…" : "Save key"}
              </button>
            </>
          )}
        </div>

        <div className="settings-section">
          <div className="settings-section-label">Spotify</div>
          <p className="hint">
            {spotifyCredsSet === true
              ? "Connected — Set Builder can match a Spotify playlist link against your library. Only the playlist's track list is read, nothing is played or downloaded, and no personal Spotify login is involved."
              : "Lets Set Builder read a public Spotify playlist's track list and match it against your library. Needs your own free Spotify Developer app (Client Credentials, no personal login) — create one at developer.spotify.com/dashboard, then paste its Client ID and Secret here."}
          </p>
          {spotifyCredsSet === true ? (
            <button type="button" onClick={() => void removeSpotifyCredentials()}>
              Remove credentials
            </button>
          ) : (
            <>
              <input
                type="text"
                autoComplete="off"
                spellCheck={false}
                className="panel-search-input"
                placeholder="Client ID"
                value={spotifyClientId}
                onChange={(e) => setSpotifyClientId(e.target.value)}
              />
              <input
                type="password"
                autoComplete="off"
                spellCheck={false}
                className="panel-search-input"
                placeholder="Client Secret"
                value={spotifyClientSecret}
                onChange={(e) => setSpotifyClientSecret(e.target.value)}
              />
              <button type="button" disabled={savingSpotify} onClick={() => void saveSpotifyCredentials()}>
                {savingSpotify ? "Saving…" : "Save credentials"}
              </button>
            </>
          )}
        </div>

        <div className="settings-section">
          <div className="settings-section-label">Play Sharing</div>
          <p className="hint">
            Sends the track, artist, BPM, key and genre of what you play to MixInspo's shared,
            anonymous play log — never your name, account or anything that identifies you. Used
            to build a real "popular with MixInspo DJs" list from actual usage. On by default.
          </p>
          <label className="toggle-row">
            <input
              type="checkbox"
              checked={sharePlays === true}
              disabled={sharePlays === null}
              onChange={(e) => void toggleSharePlays(e.target.checked)}
            />
            Share my plays
          </label>
        </div>

        <div className="settings-section">
          <div className="settings-section-label">Updates</div>
          <p className="hint">Checks GitHub for a newer version of MixInspo DJ and installs it.</p>
          <button disabled={updating} onClick={() => void checkForUpdate()}>
            Check for updates
          </button>
          {updateStatus && <p className="hint">{updateStatus}</p>}
        </div>

        <div className="settings-section">
          <div className="settings-section-label">Report a problem</div>
          <p className="hint">
            Tell us what went wrong. Only your message, the email below (if you add one) and the
            app version are sent — nothing from your library.
          </p>
          <textarea
            className="panel-search-input"
            rows={4}
            placeholder="What happened?"
            value={bugMessage}
            onChange={(e) => setBugMessage(e.target.value)}
          />
          <input
            className="panel-search-input"
            type="email"
            placeholder="Your email, if you'd like a reply (optional)"
            value={bugEmail}
            onChange={(e) => setBugEmail(e.target.value)}
          />
          <button
            disabled={sendingBug || bugMessage.trim().length < 5}
            onClick={() => void sendBugReport()}
          >
            {sendingBug ? "Sending…" : "Send report"}
          </button>
          {bugStatus && <p className="hint">{bugStatus}</p>}
        </div>

        <div className="settings-section">
          <div className="settings-section-label">On startup</div>
          <p className="hint">
            The Traktor listener and Now Playing always run in the background either way — this
            only picks which screen greets you when the app opens.
          </p>
          <select
            className="panel-search-input"
            value={startupView ?? "live"}
            disabled={startupView === null}
            onChange={(e) => void changeStartupView(e.target.value as StartupView)}
          >
            {STARTUP_VIEWS.map((v) => (
              <option key={v} value={v}>
                {STARTUP_VIEW_LABELS[v]}
              </option>
            ))}
          </select>
        </div>

        <div className="settings-section">
          <div className="settings-section-label">Window & macOS behaviour</div>
          <p className="hint">
            Mini mode, menu-bar show/hide, launch at login, and remembering window position/size are
            not built yet — a dedicated milestone, not faked here.
          </p>
        </div>
        </div>
      </div>
    </div>
  );
}
