//! Hintergrund-Threads: Vorschaubilder vorladen und behaltene Bilder speichern.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use chrono::NaiveDateTime;

use crate::imaging::{self, Edit, OutputSettings};

pub const PREVIEW_SIDE: u32 = 2048;

pub struct Preview {
    pub image: Arc<egui::ColorImage>,
    pub when: NaiveDateTime,
}

pub type LoadResult = Result<Arc<Preview>, String>;

#[derive(Default)]
struct LoaderState {
    wanted: Vec<PathBuf>,
    busy: HashSet<PathBuf>,
    done: HashMap<PathBuf, LoadResult>,
    stop: bool,
}

pub struct Loader {
    state: Arc<(Mutex<LoaderState>, Condvar)>,
    threads: Vec<JoinHandle<()>>,
}

impl Loader {
    pub fn new(ctx: egui::Context, workers: usize) -> Self {
        let state = Arc::new((Mutex::new(LoaderState::default()), Condvar::new()));
        let threads = (0..workers)
            .map(|_| {
                let state = state.clone();
                let ctx = ctx.clone();
                std::thread::spawn(move || loop {
                    let path = {
                        let (lock, cv) = &*state;
                        let mut s = lock.lock().unwrap();
                        loop {
                            if s.stop {
                                return;
                            }
                            let next = s
                                .wanted
                                .iter()
                                .find(|p| !s.busy.contains(*p) && !s.done.contains_key(*p))
                                .cloned();
                            if let Some(p) = next {
                                s.busy.insert(p.clone());
                                break p;
                            }
                            s = cv.wait(s).unwrap();
                        }
                    };
                    let res = imaging::load_preview(&path, PREVIEW_SIDE).map(|(rgb, when)| {
                        let size = [rgb.width() as usize, rgb.height() as usize];
                        Arc::new(Preview {
                            image: Arc::new(egui::ColorImage::from_rgb(size, rgb.as_raw())),
                            when,
                        })
                    });
                    let (lock, _) = &*state;
                    let mut s = lock.lock().unwrap();
                    s.busy.remove(&path);
                    if s.wanted.contains(&path) {
                        s.done.insert(path, res);
                        ctx.request_repaint();
                    }
                })
            })
            .collect();
        Self { state, threads }
    }

    /// Setzt die gewünschten Bilder (wichtigstes zuerst) und verwirft alle anderen.
    pub fn set_wanted(&self, wanted: Vec<PathBuf>) {
        let (lock, cv) = &*self.state;
        let mut s = lock.lock().unwrap();
        if s.wanted == wanted {
            return;
        }
        let keep: HashSet<&PathBuf> = wanted.iter().collect();
        s.done.retain(|k, _| keep.contains(k));
        s.wanted = wanted;
        cv.notify_all();
    }

    pub fn get(&self, path: &Path) -> Option<LoadResult> {
        self.state.0.lock().unwrap().done.get(path).cloned()
    }

    pub fn shutdown(&mut self) {
        {
            let (lock, cv) = &*self.state;
            lock.lock().unwrap().stop = true;
            cv.notify_all();
        }
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

// ------------------------------------------------------------------ Speichern

pub struct SaveJob {
    pub id: u64,
    pub path: PathBuf,
    pub edit: Edit,
    pub cfg: OutputSettings,
    pub target: PathBuf,
}

#[derive(Default)]
struct SaverShared {
    results: HashMap<u64, Result<PathBuf, String>>,
    cancelled: HashSet<u64>,
    queued: usize,
}

pub struct Saver {
    tx: Option<crossbeam_channel::Sender<SaveJob>>,
    shared: Arc<Mutex<SaverShared>>,
    threads: Vec<JoinHandle<()>>,
    next_id: u64,
}

impl Saver {
    pub fn new(ctx: egui::Context, workers: usize) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded::<SaveJob>();
        let shared = Arc::new(Mutex::new(SaverShared::default()));
        let threads = (0..workers)
            .map(|_| {
                let rx = rx.clone();
                let shared = shared.clone();
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    for job in rx {
                        if shared.lock().unwrap().cancelled.contains(&job.id) {
                            shared.lock().unwrap().queued -= 1;
                            continue;
                        }
                        let res = imaging::save(&job.path, &job.edit, &job.cfg, &job.target);
                        let mut s = shared.lock().unwrap();
                        s.queued -= 1;
                        if s.cancelled.contains(&job.id) {
                            if let Ok(p) = &res {
                                let _ = std::fs::remove_file(p);
                            }
                        } else {
                            s.results.insert(job.id, res);
                        }
                        ctx.request_repaint();
                    }
                })
            })
            .collect();
        Self {
            tx: Some(tx),
            shared,
            threads,
            next_id: 1,
        }
    }

    pub fn submit(
        &mut self,
        path: PathBuf,
        edit: Edit,
        cfg: OutputSettings,
        target: PathBuf,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.shared.lock().unwrap().queued += 1;
        if let Some(tx) = &self.tx {
            let _ = tx.send(SaveJob {
                id,
                path,
                edit,
                cfg,
                target,
            });
        }
        id
    }

    /// Rückgängig: Auftrag abbrechen bzw. bereits geschriebene Datei löschen.
    pub fn cancel(&self, id: u64) {
        let mut s = self.shared.lock().unwrap();
        s.cancelled.insert(id);
        if let Some(Ok(p)) = s.results.remove(&id) {
            let _ = std::fs::remove_file(p);
        }
    }

    pub fn take_results(&self) -> Vec<(u64, Result<PathBuf, String>)> {
        self.shared.lock().unwrap().results.drain().collect()
    }

    pub fn queued(&self) -> usize {
        self.shared.lock().unwrap().queued
    }

    /// Wartet, bis alle Aufträge erledigt sind.
    pub fn finish(&mut self) -> Vec<(u64, Result<PathBuf, String>)> {
        self.tx = None;
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
        self.take_results()
    }
}
