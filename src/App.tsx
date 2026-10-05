import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./App.css";

type ScanProgress = {
  current: number;
  total: number;
  current_file: string;
};

type ScanSummary = {
  folder: string;
  files_found: number;
  inserted: number;
  updated: number;
  duplicates_found: number;
  errors: number;
};

function App() {
  const [dbStatus, setDbStatus] = useState<string>("connecting…");
  const [folder, setFolder] = useState<string | null>(null);
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [summary, setSummary] = useState<ScanSummary | null>(null);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<string>("db_health_check")
      .then(setDbStatus)
      .catch((e) => setDbStatus(`error: ${e}`));
  }, []);

  useEffect(() => {
    const unlisten = listen<ScanProgress>("scan-progress", (event) => {
      setProgress(event.payload);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  async function chooseFolder() {
    setError(null);
    const picked = await invoke<string | null>("pick_music_folder");
    if (!picked) return;
    setFolder(picked);
    setSummary(null);
    setProgress(null);
    setScanning(true);
    try {
      const result = await invoke<ScanSummary>("scan_library_folder", { folder: picked });
      setSummary(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  }

  return (
    <main className="app">
      <header className="app-header">
        <h1>MixFlow</h1>
        <span className="db-status">{dbStatus}</span>
      </header>

      <section className="panel">
        <h2>Library</h2>
        <p className="hint">Select a folder of music files to scan into your library.</p>

        <button onClick={chooseFolder} disabled={scanning}>
          {scanning ? "Scanning…" : "Select Music Folder"}
        </button>

        {folder && <p className="folder-path">{folder}</p>}

        {scanning && progress && (
          <div className="progress">
            <div className="progress-bar">
              <div
                className="progress-bar-fill"
                style={{
                  width: progress.total > 0 ? `${(progress.current / progress.total) * 100}%` : "0%",
                }}
              />
            </div>
            <p className="progress-label">
              {progress.current} / {progress.total} — {progress.current_file}
            </p>
          </div>
        )}

        {error && <p className="error">{error}</p>}

        {summary && (
          <div className="summary">
            <div className="summary-row">
              <span>Files found</span>
              <span>{summary.files_found}</span>
            </div>
            <div className="summary-row">
              <span>Added</span>
              <span>{summary.inserted}</span>
            </div>
            <div className="summary-row">
              <span>Updated</span>
              <span>{summary.updated}</span>
            </div>
            <div className="summary-row">
              <span>Possible duplicates</span>
              <span>{summary.duplicates_found}</span>
            </div>
            <div className="summary-row">
              <span>Errors</span>
              <span>{summary.errors}</span>
            </div>
          </div>
        )}
      </section>
    </main>
  );
}

export default App;
