//! Exports one set as a branded PDF: a MixInspo page with the tracks in order,
//! how long each played, the key and BPM, and the set's headline numbers.
//!
//! The PDF is drawn directly (printpdf, built-in Helvetica), so it needs no
//! browser or external tools. The user picks where the file is saved.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;

use mixflow_db::plays;
use printpdf::{BuiltinFont, Color, IndirectFontRef, Mm, PdfDocument, PdfDocumentReference, PdfLayerReference, Point, Polygon, Rgb};
use rusqlite::Connection;
use tauri::State;

use crate::traktor_history;
use crate::DbState;

const PAGE_W: f32 = 210.0;
const PAGE_H: f32 = 297.0;
const MARGIN: f32 = 18.0;
/// Brand colours: the same cyan and near-black as the app.
const CYAN: (f32, f32, f32) = (0.153, 0.757, 0.827);
const INK: (f32, f32, f32) = (0.043, 0.047, 0.055);
const MUTED: (f32, f32, f32) = (0.357, 0.384, 0.424);
const RULE: (f32, f32, f32) = (0.84, 0.86, 0.89);

struct Row {
    number: usize,
    title: String,
    artist: String,
    bpm: String,
    key: String,
    started: String,
    played: String,
}

struct SetDocument {
    title: String,
    subtitle: String,
    stats: Vec<(String, String)>,
    rows: Vec<Row>,
    note: String,
}

/// Exports one set ("app" for a session this app recorded, "traktor" for an
/// imported Traktor session). Returns the saved path, or None if the user cancelled.
#[tauri::command]
pub fn export_set_pdf(db: State<DbState>, kind: String, id: i64) -> Result<Option<String>, String> {
    let document = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        match kind.as_str() {
            "app" => app_set_document(&conn, id)?,
            "traktor" => traktor_set_document(&conn, id)?,
            other => return Err(format!("Unknown set type: {other}")),
        }
    };

    let Some(target) = rfd::FileDialog::new()
        .set_title("Save set as PDF")
        .set_file_name(format!("{}.pdf", file_safe(&document.title)))
        .add_filter("PDF", &["pdf"])
        .save_file()
    else {
        return Ok(None);
    };

    write_pdf(&document, &target)?;
    Ok(Some(target.to_string_lossy().into_owned()))
}

fn app_set_document(conn: &Connection, session_id: i64) -> Result<SetDocument, String> {
    let plays = plays::session_plays(conn, session_id).map_err(|e| e.to_string())?;
    let started = plays
        .first()
        .map(|p| friendly_timestamp(&p.played_at))
        .unwrap_or_else(|| "No plays".to_string());
    let total: i64 = plays.iter().filter_map(|p| p.est_seconds).sum();
    let known = plays.iter().filter(|p| p.est_seconds.is_some()).count();
    let suggested = plays.iter().filter(|p| p.source == "recommended").count();
    let share = if plays.is_empty() {
        0
    } else {
        (suggested * 100) / plays.len()
    };
    let most = most_played(plays.iter().map(|p| (Some(p.track_id), p.title.clone())));

    let rows = plays
        .iter()
        .map(|p| Row {
            number: p.position as usize,
            title: p.title.clone().unwrap_or_else(|| "Untitled".into()),
            artist: p.artist.clone().unwrap_or_else(|| "Unknown artist".into()),
            bpm: p.bpm.map(|b| format!("{}", b.round() as i64)).unwrap_or_default(),
            key: p.camelot_key.clone().unwrap_or_default(),
            started: clock_from_timestamp(&p.played_at),
            played: p.est_seconds.map(minutes).unwrap_or_else(|| "—".into()),
        })
        .collect();

    Ok(SetDocument {
        title: format!("Set · {started}"),
        subtitle: format!("{} played in MixInspo", count_tracks(plays.len())),
        stats: vec![
            ("Tracks".into(), plays.len().to_string()),
            ("Playing time".into(), if known == 0 { "—".into() } else { minutes(total) }),
            ("From suggestions".into(), format!("{share}%")),
            ("Most played".into(), most),
        ],
        rows,
        note: "Playing times are estimated from when each track was followed by the next, \
               so they can run over if a track rang out. The last track has no estimate."
            .into(),
    })
}

fn traktor_set_document(conn: &Connection, set_id: i64) -> Result<SetDocument, String> {
    let plays = traktor_history::report_rows(conn, set_id)?;
    let total: f64 = plays.iter().map(|p| p.played_seconds).sum();
    let matched = plays.iter().filter(|p| p.track_id.is_some()).count();
    let most = most_played(plays.iter().map(|p| (p.track_id, Some(p.title.clone()))));
    let when = conn
        .query_row(
            "SELECT file_name FROM traktor_history_sets WHERE id = ?1",
            rusqlite::params![set_id],
            |row| row.get::<_, String>(0),
        )
        .map(|name| session_time(&name))
        .unwrap_or_else(|_| "Traktor session".into());

    let rows = plays
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let (bpm, key) = bpm_and_key(conn, p.track_id);
            Row {
                number: i + 1,
                title: p.title.clone(),
                artist: p.artist.clone().unwrap_or_else(|| "Not in your library".into()),
                bpm,
                key,
                started: clock(p.start_seconds),
                played: minutes(p.played_seconds.round() as i64),
            }
        })
        .collect();

    Ok(SetDocument {
        title: format!("Traktor session · {when}"),
        subtitle: format!("{}, from Traktor's own history", count_tracks(plays.len())),
        stats: vec![
            ("Tracks".into(), plays.len().to_string()),
            ("Playing time".into(), minutes(total.round() as i64)),
            ("In your library".into(), format!("{matched} of {}", plays.len())),
            ("Most played".into(), most),
        ],
        rows,
        note: "Playing times are exact, from Traktor's own history.".into(),
    })
}

fn bpm_and_key(conn: &Connection, track_id: Option<i64>) -> (String, String) {
    let Some(id) = track_id else {
        return (String::new(), String::new());
    };
    conn.query_row(
        "SELECT bpm, camelot_key FROM tracks WHERE id = ?1",
        rusqlite::params![id],
        |row| {
            let bpm: Option<f64> = row.get(0)?;
            let key: Option<String> = row.get(1)?;
            Ok((bpm.map(|b| format!("{}", b.round() as i64)).unwrap_or_default(), key.unwrap_or_default()))
        },
    )
    .unwrap_or_default()
}

/// The most-played track as "Title (3×)", or "—" when nothing repeats.
fn most_played(plays: impl Iterator<Item = (Option<i64>, Option<String>)>) -> String {
    let mut counts: HashMap<i64, (usize, String)> = HashMap::new();
    for (track, title) in plays {
        let Some(id) = track else { continue };
        let entry = counts.entry(id).or_insert((0, title.clone().unwrap_or_else(|| "Untitled".into())));
        entry.0 += 1;
    }
    match counts.values().max_by_key(|(n, _)| *n) {
        Some((n, title)) if *n > 1 => format!("{title} ({n}×)"),
        _ => "—".into(),
    }
}

fn minutes(seconds: i64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn clock(seconds_since_midnight: i64) -> String {
    format!("{:02}:{:02}", (seconds_since_midnight / 3600) % 24, (seconds_since_midnight % 3600) / 60)
}

/// "6 tracks" or "1 track".
fn count_tracks(n: usize) -> String {
    if n == 1 {
        "1 track".into()
    } else {
        format!("{n} tracks")
    }
}

/// "2026-10-06 08:27:07" becomes "6 Oct 2026, 08:27".
fn friendly_timestamp(timestamp: &str) -> String {
    let parts: Vec<&str> = timestamp.split(|c| c == '-' || c == ' ' || c == ':').collect();
    if parts.len() < 5 {
        return timestamp.to_string();
    }
    let month = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
        .get(parts[1].parse::<usize>().unwrap_or(0).saturating_sub(1))
        .copied()
        .unwrap_or("");
    format!("{} {} {}, {}:{}", parts[2].trim_start_matches('0'), month, parts[0], parts[3], parts[4])
}

/// "2026-10-06 20:15:03" becomes "20:15".
fn clock_from_timestamp(timestamp: &str) -> String {
    timestamp.get(11..16).unwrap_or("").to_string()
}

/// "history_2026y10m06d_08h19m42s.nml" becomes "2026-10-06 08:19".
fn session_time(file_name: &str) -> String {
    let digits: String = file_name.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 12 {
        return file_name.to_string();
    }
    format!(
        "{}-{}-{} {}:{}",
        &digits[0..4],
        &digits[4..6],
        &digits[6..8],
        &digits[8..10],
        &digits[10..12]
    )
}

fn file_safe(title: &str) -> String {
    title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '-' })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Writes the set as a branded A4 PDF, adding pages as the track list grows.
fn write_pdf(data: &SetDocument, target: &std::path::Path) -> Result<(), String> {
    let (doc, page, layer) = PdfDocument::new("MixInspo set", Mm(PAGE_W), Mm(PAGE_H), "page");
    let regular = doc.add_builtin_font(BuiltinFont::Helvetica).map_err(|e| e.to_string())?;
    let bold = doc.add_builtin_font(BuiltinFont::HelveticaBold).map_err(|e| e.to_string())?;
    let first_layer = doc.get_page(page).get_layer(layer);
    let mut pdf = Pdf { doc, regular, bold, layer: first_layer, pages: 1 };
    pdf.footer();

    pdf.header(data);
    let mut y = pdf.stats(data, 226.0);
    y = pdf.table_header(y);
    for row in &data.rows {
        if y < 34.0 {
            pdf.new_page();
            y = pdf.table_header(278.0);
        }
        y = pdf.row(row, y);
    }
    if y < 42.0 {
        pdf.new_page();
        y = 278.0;
    }
    pdf.text(&data.note, MARGIN, y - 6.0, 7.5, false, MUTED);

    let file = File::create(target).map_err(|e| e.to_string())?;
    pdf.doc
        .save(&mut BufWriter::new(file))
        .map_err(|e| e.to_string())
}

struct Pdf {
    doc: PdfDocumentReference,
    regular: IndirectFontRef,
    bold: IndirectFontRef,
    layer: PdfLayerReference,
    pages: usize,
}

impl Pdf {
    fn new_page(&mut self) {
        let (page, layer) = self.doc.add_page(Mm(PAGE_W), Mm(PAGE_H), "page");
        self.layer = self.doc.get_page(page).get_layer(layer);
        self.pages += 1;
        self.footer();
    }

    fn colour(&self, (r, g, b): (f32, f32, f32)) {
        self.layer.set_fill_color(Color::Rgb(Rgb::new(r, g, b, None)));
    }

    fn text(&self, text: &str, x: f32, y: f32, size: f32, bold: bool, colour: (f32, f32, f32)) {
        self.colour(colour);
        let font = if bold { &self.bold } else { &self.regular };
        self.layer.use_text(text, size, Mm(x), Mm(y), font);
    }

    /// Text right-aligned so it ends at `right` millimetres.
    fn text_right(&self, text: &str, right: f32, y: f32, size: f32, bold: bool, colour: (f32, f32, f32)) {
        let x = right - approx_width(text, size);
        self.text(text, x, y, size, bold, colour);
    }

    fn rect(&self, x: f32, y: f32, w: f32, h: f32, colour: (f32, f32, f32)) {
        self.colour(colour);
        let points = vec![
            (Point::new(Mm(x), Mm(y)), false),
            (Point::new(Mm(x + w), Mm(y)), false),
            (Point::new(Mm(x + w), Mm(y + h)), false),
            (Point::new(Mm(x), Mm(y + h)), false),
        ];
        self.layer.add_polygon(Polygon {
            rings: vec![points],
            mode: printpdf::path::PaintMode::Fill,
            winding_order: printpdf::path::WindingOrder::NonZero,
        });
    }

    fn rule(&self, x1: f32, x2: f32, y: f32, thickness: f32, colour: (f32, f32, f32)) {
        self.rect(x1, y, x2 - x1, thickness, colour);
    }

    fn header(&self, data: &SetDocument) {
        // Brand: a near-black badge with a cyan M, the name beside it, and a cyan rule under.
        self.rect(MARGIN, 281.0, 9.0, 9.0, INK);
        self.text("M", MARGIN + 2.6, 283.2, 11.0, true, CYAN);
        self.text("MixInspo DJ", MARGIN + 12.0, 283.4, 13.0, true, INK);
        self.rule(MARGIN, PAGE_W - MARGIN, 277.5, 0.9, CYAN);

        self.text(&truncate(&data.title, 60), MARGIN, 258.0, 18.0, true, INK);
        self.text(&truncate(&data.subtitle, 90), MARGIN, 250.5, 10.0, false, MUTED);
    }

    /// Writes the headline numbers across one row; returns the y to continue from.
    fn stats(&self, data: &SetDocument, y: f32) -> f32 {
        let width = (PAGE_W - 2.0 * MARGIN) / data.stats.len().max(1) as f32;
        for (i, (label, value)) in data.stats.iter().enumerate() {
            let x = MARGIN + width * i as f32;
            self.text(&label.to_uppercase(), x, y, 7.0, false, MUTED);
            self.text(&truncate(value, 26), x, y - 7.0, 12.0, true, INK);
        }
        y - 16.0
    }

    /// Column headings for the track list; returns the y of the first row.
    fn table_header(&self, y: f32) -> f32 {
        self.text("#", MARGIN, y, 7.0, false, MUTED);
        self.text("TRACK", MARGIN + 8.0, y, 7.0, false, MUTED);
        self.text_right("BPM", PAGE_W - MARGIN - 52.0, y, 7.0, false, MUTED);
        self.text_right("KEY", PAGE_W - MARGIN - 36.0, y, 7.0, false, MUTED);
        self.text_right("STARTED", PAGE_W - MARGIN - 19.0, y, 7.0, false, MUTED);
        self.text_right("PLAYED", PAGE_W - MARGIN, y, 7.0, false, MUTED);
        self.rule(MARGIN, PAGE_W - MARGIN, y - 2.5, 0.3, RULE);
        y - 8.0
    }

    fn row(&self, row: &Row, y: f32) -> f32 {
        self.text(&row.number.to_string(), MARGIN, y, 8.0, false, MUTED);
        self.text(&truncate(&row.title, 46), MARGIN + 8.0, y, 9.0, true, INK);
        self.text(&truncate(&row.artist, 52), MARGIN + 8.0, y - 4.2, 7.5, false, MUTED);
        self.text_right(&row.bpm, PAGE_W - MARGIN - 52.0, y, 9.0, false, INK);
        self.key_chip(&row.key, PAGE_W - MARGIN - 36.0, y);
        self.text_right(&row.started, PAGE_W - MARGIN - 19.0, y, 9.0, false, INK);
        self.text_right(&row.played, PAGE_W - MARGIN, y, 9.0, true, INK);
        self.rule(MARGIN, PAGE_W - MARGIN, y - 7.0, 0.2, RULE);
        y - 11.0
    }

    /// A key as a small cyan-tinted chip, right-aligned to `right`.
    fn key_chip(&self, key: &str, right: f32, y: f32) {
        if key.is_empty() {
            self.text_right("—", right, y, 9.0, false, MUTED);
            return;
        }
        let width = approx_width(key, 8.0) + 3.0;
        self.rect(right - width, y - 1.6, width, 4.6, (0.90, 0.98, 0.99));
        self.text_right(key, right - 1.5, y, 8.0, true, (0.04, 0.44, 0.48));
    }

    fn footer(&self) {
        let label = format!("MixInspo DJ · page {}", self.pages);
        self.text_right(&label, PAGE_W - MARGIN, 10.0, 7.0, false, MUTED);
    }
}

/// A rough width in millimetres for a Helvetica string, good enough for alignment.
fn approx_width(text: &str, size: f32) -> f32 {
    text.chars().count() as f32 * size * 0.52 * 0.3528
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let kept: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{kept}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn session_file_names_read_as_a_time() {
        assert_eq!(session_time("history_2026y10m06d_08h19m42s.nml"), "2026-10-06 08:19");
    }

    #[test]
    fn timestamps_and_counts_read_naturally() {
        assert_eq!(friendly_timestamp("2026-10-06 08:27:07"), "6 Oct 2026, 08:27");
        assert_eq!(count_tracks(1), "1 track");
        assert_eq!(count_tracks(3), "3 tracks");
    }

    #[test]
    fn clock_and_minutes_format() {
        assert_eq!(clock(29293), "08:08");
        assert_eq!(minutes(216), "3:36");
    }

    #[test]
    fn a_set_is_written_as_a_real_pdf() {
        let doc = SetDocument {
            title: "Traktor session · 2026-10-06 08:19".into(),
            subtitle: "3 tracks".into(),
            stats: vec![("Tracks".into(), "3".into()), ("Most played".into(), "—".into())],
            rows: (1..=3)
                .map(|n| Row {
                    number: n,
                    title: format!("Tom & Jerry <Live> {n}"),
                    artist: "Earth, Wind & Fire".into(),
                    bpm: "126".into(),
                    key: "11B".into(),
                    started: "08:08".into(),
                    played: "3:36".into(),
                })
                .collect(),
            note: "Exact times.".into(),
        };
        let dir = std::env::temp_dir().join("mixinspo-export-test");
        fs::create_dir_all(&dir).unwrap();
        let pdf = dir.join("set.pdf");
        write_pdf(&doc, &pdf).unwrap();
        let bytes = fs::read(&pdf).unwrap();
        assert!(bytes.starts_with(b"%PDF"), "output should be a PDF");
    }
}

