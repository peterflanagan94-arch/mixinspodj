import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { AnalyzeProgress, AnalyzeSummary, ScanProgress, ScanSummary, TraktorStatus } from "../types";

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
    if (open) void loadWatchedFolders();
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
            Status:{" "}
            {traktorStatus?.state === "connected" ? (
              <strong className="traktor-connected">Traktor is connected</strong>
            ) : traktorStatus?.state === "listening"
                ? "waiting for Traktor"
                : "starting…"}
          </p>
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
