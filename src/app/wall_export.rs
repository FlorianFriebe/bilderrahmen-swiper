//! Fotowand, Schritt 3: Druckdateien speichern. Jedes Foto als bearbeitete Kopie in echter
//! Abzugsgröße (300 dpi), nummeriert, verdeckte Teile je nach Einstellung weiß mit
//! Schnittlinie; dazu Aufhängeplan.png und Einkaufsliste.txt. Originale bleiben unberührt,
//! ein früherer Stand wandert in den Papierkorb des Ordners.

use super::*;
use crate::wall::{self as wl, Export};

/// Druckdateien werden im Hintergrund geschrieben: (fertig, gesamt, Ergebnis)
pub type WallExport = Arc<Mutex<(usize, usize, Option<ExportDone>)>>;

/// Ergebnis: Ordner, Anzahl gespeichert, Fotos, die nicht gingen.
pub struct ExportDone {
    pub dir: PathBuf,
    pub saved: usize,
    pub failed: Vec<String>,
}

/// Ein Abzug, fertig beschrieben für den Hintergrund-Thread.
struct Item {
    src: PathBuf,
    name: String,
    number: usize,
    /// Lage an der Wand in cm
    at: (f32, f32, f32, f32),
    border: f32,
    focus: (f32, f32),
    mask: wl::Mask,
    /// sichtbare Ecke für die Nummer im Aufhängeplan
    spot: (f32, f32),
}

/// Ordner der Druckdateien innerhalb des Export-Ordners.
pub const PRINT_DIR: &str = "Druck";
pub const PLAN_FILE: &str = "Aufhängeplan.png";
pub const LIST_FILE: &str = "Einkaufsliste.txt";

impl SwiperApp {
    /// Ordner für die fertige Planung: „Fotowand – <Name>“ neben dem Zielordner.
    pub fn wall_export_dir(&self) -> PathBuf {
        let t = &self.cfg.target;
        let base = t
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(t);
        base.join(format!("Fotowand – {}", wl::safe_name(&self.wall().name)))
    }

    /// Alles für Schritt 3 speichern. Läuft im Hintergrund; `poll_wall_export` meldet.
    pub fn wall_export(&mut self) {
        if self.wall_export.is_some() {
            return;
        }
        let w = self.wall().clone();
        let sizes = self.sizes().to_vec();
        let nums = wl::numbers(&w, &sizes);
        let total = nums.iter().flatten().count();
        if total == 0 {
            self.toast("Noch kein Foto an der Wand.".into());
            return;
        }
        let e = w.export.clone();
        let mut items = vec![];
        for (i, p) in w.pieces.iter().enumerate() {
            let (Some(src), Some(number)) = (p.photo.clone(), nums[i]) else {
                continue;
            };
            let a = p.area(&sizes);
            items.push(Item {
                name: wl::file_name(number, total, p, &sizes),
                number,
                at: (a.x, a.y, a.w, a.h),
                border: w.border_of(&sizes, p),
                focus: w.photo(&src).map(|f| f.focus).unwrap_or_default(),
                mask: wl::mask(&w, &sizes, i, e.reserve_mm),
                spot: wl::visible_spot(&w, &sizes, i),
                src,
            });
        }
        items.sort_by_key(|it| it.number);
        let dir = self.wall_export_dir();
        trash_old(&dir);
        let list = wl::shopping_list(&w, &sizes, &nums);
        let title = format!("Fotowand „{}“", w.name);
        let (ww, wh) = (w.w, w.h);
        let clip = w.trims().then(|| {
            let a = w.inner();
            (a.x, a.y, a.w, a.h)
        });
        let state: WallExport = Arc::new(Mutex::new((0, items.len(), None)));
        let st = state.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let done = write_all(&dir, &items, &e, &list, &title, (ww, wh, clip), &st, &ctx);
            if let Ok(mut s) = st.lock() {
                s.2 = Some(done);
            }
            ctx.request_repaint();
        });
        self.wall_export = Some(state);
    }

    /// Fortschritt beim Speichern (0..1), solange es läuft.
    pub fn wall_export_progress(&self) -> Option<f32> {
        let s = self.wall_export.as_ref()?.lock().ok()?;
        Some(s.0 as f32 / s.1.max(1) as f32)
    }

    pub fn poll_wall_export(&mut self) {
        let done = self
            .wall_export
            .as_ref()
            .and_then(|s| s.lock().ok().and_then(|mut s| s.2.take()));
        let Some(d) = done else { return };
        self.wall_export = None;
        let folder = crate::folders::file_name(&d.dir);
        let msg = if d.failed.is_empty() {
            format!("{} in „{folder}“ gespeichert", druckdateien(d.saved))
        } else {
            format!(
                "{} gespeichert, {} nicht: {}",
                druckdateien(d.saved),
                d.failed.len(),
                d.failed.join(", ")
            )
        };
        self.toast(msg);
        self.wall_ui.exported = Some(d.dir);
    }
}

fn druckdateien(n: usize) -> String {
    if n == 1 {
        "1 Druckdatei".into()
    } else {
        format!("{n} Druckdateien")
    }
}

/// Früheren Stand in den Papierkorb des Ordners legen (14 Tage aufbewahrt).
fn trash_old(dir: &Path) {
    crate::history::empty_old_trash(dir);
    let mut old: Vec<PathBuf> = std::fs::read_dir(dir.join(PRINT_DIR))
        .map(|r| {
            r.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect()
        })
        .unwrap_or_default();
    old.extend(
        [PLAN_FILE, LIST_FILE]
            .iter()
            .map(|f| dir.join(f))
            .filter(|p| p.is_file()),
    );
    for f in old {
        crate::history::trash_file(dir, &f, false);
    }
}

/// Datei sicher schreiben (erst unter anderem Namen, dann umbenennen).
fn write_file(path: &Path, data: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("part");
    std::fs::write(&tmp, data)
        .and_then(|_| std::fs::rename(&tmp, path))
        .map_err(|e| e.to_string())
}

#[allow(clippy::too_many_arguments)]
fn write_all(
    dir: &Path,
    items: &[Item],
    e: &Export,
    list: &str,
    title: &str,
    wall: (f32, f32, Option<imaging::CmRect>),
    st: &WallExport,
    ctx: &egui::Context,
) -> ExportDone {
    let mut out = ExportDone {
        dir: dir.to_path_buf(),
        saved: 0,
        failed: vec![],
    };
    let prints = dir.join(PRINT_DIR);
    if let Err(err) = std::fs::create_dir_all(&prints) {
        out.failed
            .push(format!("Ordner {}: {err}", prints.display()));
        return out;
    }
    let mut plan = vec![];
    for (k, it) in items.iter().enumerate() {
        let (_, _, w, h) = it.at;
        let (pw, ph) = (wl::px_of(w), wl::px_of(h));
        let res = imaging::decode(&it.src).map(|d| imaging::to_rgb(d.img));
        let mut small = None;
        match res {
            Ok(src) => {
                let job = imaging::PrintJob {
                    w: pw,
                    h: ph,
                    border: wl::px_of(it.border).min(pw / 3) * u32::from(it.border > 0.0),
                    focus: it.focus,
                    mask: &it.mask,
                    export: e,
                    number: e.number.then(|| it.number.to_string()),
                };
                let img = imaging::render_print(&src, &job);
                small = Some(imaging::resize(
                    &img,
                    (pw / 8).max(1),
                    (ph / 8).max(1),
                    true,
                ));
                match imaging::encode_print(&img)
                    .and_then(|data| write_file(&prints.join(&it.name), &data))
                {
                    Ok(()) => out.saved += 1,
                    Err(_) => out.failed.push(crate::folders::file_name(&it.src)),
                }
            }
            Err(_) => out.failed.push(crate::folders::file_name(&it.src)),
        }
        plan.push(imaging::PlanItem {
            at: it.at,
            img: small,
            number: Some(it.number),
            spot: it.spot,
        });
        if let Ok(mut s) = st.lock() {
            s.0 = k + 1;
        }
        ctx.request_repaint();
    }
    let img = imaging::render_plan(wall.0, wall.1, title, &plan, wall.2);
    let mut png = std::io::Cursor::new(vec![]);
    let saved = img
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|e| e.to_string())
        .and_then(|_| write_file(&dir.join(PLAN_FILE), png.get_ref()));
    if saved.is_err() {
        out.failed.push(PLAN_FILE.into());
    }
    // mit BOM, damit der Windows-Editor die Umlaute richtig zeigt
    let text = format!("\u{feff}{list}");
    if write_file(&dir.join(LIST_FILE), text.as_bytes()).is_err() {
        out.failed.push(LIST_FILE.into());
    }
    out
}
