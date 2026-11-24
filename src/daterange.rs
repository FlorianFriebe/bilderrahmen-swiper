//! Zeitraum-Filter („nur Fotos aus 2024“) und der Speicher für Aufnahmedaten,
//! damit die Daten nicht bei jedem Start neu gelesen werden müssen.

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{Datelike, NaiveDateTime};
use serde::{Deserialize, Serialize};

use crate::app::SwiperApp;
use crate::folders;
use crate::workers::DateScan;

/// Monatsgenauer Zeitraum (jeweils einschließlich).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct DateRange {
    /// (Jahr, Monat 1..=12)
    pub from: (i32, u32),
    pub to: (i32, u32),
}

pub const MONTHS: [&str; 12] = [
    "Januar",
    "Februar",
    "März",
    "April",
    "Mai",
    "Juni",
    "Juli",
    "August",
    "September",
    "Oktober",
    "November",
    "Dezember",
];

impl DateRange {
    pub fn year(y: i32) -> Self {
        Self {
            from: (y, 1),
            to: (y, 12),
        }
    }

    pub fn is_year(&self) -> Option<i32> {
        (self.from.0 == self.to.0 && self.from.1 == 1 && self.to.1 == 12).then_some(self.from.0)
    }

    pub fn contains(&self, d: NaiveDateTime) -> bool {
        let k = (d.year(), d.month());
        k >= self.from && k <= self.to
    }

    /// „2024“, „Juli 2024“, „Juni 2023 – August 2023“
    pub fn label(&self) -> String {
        if let Some(y) = self.is_year() {
            return y.to_string();
        }
        let m = |(y, m): (i32, u32)| format!("{} {y}", MONTHS[(m as usize - 1).min(11)]);
        if self.from == self.to {
            m(self.from)
        } else if self.from.0 == self.to.0 {
            format!(
                "{} – {} {}",
                MONTHS[(self.from.1 as usize - 1).min(11)],
                MONTHS[(self.to.1 as usize - 1).min(11)],
                self.to.0
            )
        } else {
            format!("{} – {}", m(self.from), m(self.to))
        }
    }

    /// Von und Bis in die richtige Reihenfolge bringen.
    pub fn normalized(mut self) -> Self {
        if self.from > self.to {
            std::mem::swap(&mut self.from, &mut self.to);
        }
        self
    }
}

fn cache_path() -> PathBuf {
    crate::system::data_dir().join("aufnahmedaten.txt")
}

/// Gespeicherte Aufnahmedaten laden (Pfad, Unix-Zeit, aus EXIF?).
pub fn load_cache() -> HashMap<PathBuf, (NaiveDateTime, bool)> {
    let Ok(text) = std::fs::read_to_string(cache_path()) else {
        return HashMap::new();
    };
    text.lines()
        .filter_map(|l| {
            let mut it = l.rsplitn(3, '\t');
            let exif = it.next()? == "1";
            let t: i64 = it.next()?.parse().ok()?;
            let p = PathBuf::from(it.next()?);
            let dt = chrono::DateTime::from_timestamp(t, 0)?.naive_utc();
            Some((p, (dt, exif)))
        })
        .collect()
}

pub fn save_cache(dates: &HashMap<PathBuf, (NaiveDateTime, bool)>) {
    let path = cache_path();
    let tmp = path.with_extension("tmp");
    let write = || -> std::io::Result<()> {
        let mut f = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        for (p, (t, exif)) in dates {
            let Some(s) = p.to_str() else { continue };
            if s.contains('\n') {
                continue;
            }
            writeln!(
                f,
                "{s}\t{}\t{}",
                t.and_utc().timestamp(),
                if *exif { 1 } else { 0 }
            )?;
        }
        f.flush()?;
        drop(f);
        std::fs::rename(&tmp, &path)
    };
    if let Err(e) = write() {
        crate::system::log(&format!("Aufnahmedaten nicht gespeichert: {e}"));
    }
}

impl SwiperApp {
    /// Liegt das Foto im gewählten Zeitraum? Unbekannte Daten zählen vorerst mit.
    pub fn in_range(&self, p: &Path) -> bool {
        self.in_range_of(self.cfg.date_filter, p)
    }

    /// Wie `in_range`, aber mit einem beliebigen Zeitraum (z. B. dem einer Session).
    pub fn in_range_of(&self, r: Option<DateRange>, p: &Path) -> bool {
        match (r, self.dates.get(p)) {
            (Some(r), Some((t, _))) => r.contains(*t),
            _ => true,
        }
    }

    pub fn set_date_filter(&mut self, r: Option<DateRange>) {
        self.cfg.date_filter = r.map(DateRange::normalized);
        self.refresh_folders();
    }

    /// Fehlende Aufnahmedaten aller Ordner im Hintergrund lesen.
    pub fn start_library_scan(&mut self, files: &[PathBuf]) {
        if self.lib_scan.is_some() {
            return;
        }
        let missing: Vec<PathBuf> = files
            .iter()
            .filter(|p| !self.dates.contains_key(*p))
            .cloned()
            .collect();
        if !missing.is_empty() {
            self.lib_scan = Some(DateScan::start(self.ctx.clone(), missing));
        }
    }

    pub fn poll_library_scan(&mut self) {
        let Some(scan) = &self.lib_scan else { return };
        if let Some(res) = scan.try_take() {
            for (p, t, exif) in res {
                self.dates.insert(p, (t, exif));
            }
            self.lib_scan = None;
            save_cache(&self.dates);
            self.refresh_folders();
        }
    }

    /// Fortschritt beim Lesen der Aufnahmedaten (0..1), falls es läuft.
    pub fn library_scan_progress(&self) -> Option<f32> {
        self.lib_scan.as_ref().map(|s| {
            let (a, b) = s.progress();
            if b == 0 {
                0.0
            } else {
                a as f32 / b as f32
            }
        })
    }

    /// Jahre mit Fotos (neueste zuerst) und deren Anzahl.
    pub fn years_from(&self, files: &[PathBuf]) -> Vec<(i32, usize)> {
        let mut m: BTreeMap<i32, usize> = BTreeMap::new();
        for p in files {
            if let Some((t, _)) = self.dates.get(p) {
                *m.entry(t.year()).or_default() += 1;
            }
        }
        m.into_iter().rev().collect()
    }
}

/// Alle Bilder eines Ordners, die im Zeitraum liegen.
pub fn filtered(app: &SwiperApp, folder: &Path) -> Vec<PathBuf> {
    folders::images_in(folder)
        .into_iter()
        .filter(|p| app.in_range(p))
        .collect()
}
