import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ScanProgress, ScanSummary } from "../types";

type Props = {
  open: boolean;
  onClose: () => void;
  onScanComplete: () => void;
};

export function SettingsPanel({ open, onClose, onScanComplete }: Props) {
  const [folder, setFolder] = useState<string | null>(null);
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [summary, setSummary] = useState<ScanSummary | null>(null);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
      onScanComplete();
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
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

        <div className="settings-section">
          <div className="settings-section-label">Library</div>
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
  );
}
