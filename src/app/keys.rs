//! Tastenbelegung: Abfragen, Beschriftungen, Neu belegen.

use super::*;

impl SwiperApp {
    pub fn keys_for(&self, action: &str) -> Vec<egui::Key> {
        let names: Vec<String> = match self.cfg.keys.get(action) {
            Some(v) => v.clone(),
            None => ACTIONS
                .iter()
                .find(|a| a.0 == action)
                .map(|a| a.2.iter().map(|s| s.to_string()).collect())
                .unwrap_or_default(),
        };
        names
            .iter()
            .filter_map(|n| egui::Key::from_name(n))
            .collect()
    }

    pub fn pressed(&self, ctx: &egui::Context, action: &str) -> bool {
        let keys = self.keys_for(action);
        ctx.input(|i| {
            !i.modifiers.command && !i.modifiers.alt && keys.iter().any(|k| i.key_pressed(*k))
        })
    }

    /// Kurzbeschriftung der ersten Taste, z. B. "→" oder "R".
    pub fn key_label(&self, action: &str) -> String {
        self.keys_for(action)
            .first()
            .map(|k| key_name(*k))
            .unwrap_or_else(|| "–".into())
    }

    /// Sehr kurze Beschriftung der ersten Taste für die Knöpfe unten, z. B. "⌫" oder "R".
    /// Wie `key_short`, aber leer, solange die Taste nicht geändert wurde
    /// (z. B. für die Pfeil-Knöpfe, deren Symbol die Taste schon zeigt).
    pub fn key_changed(&self, action: &str) -> String {
        let default: Vec<String> = ACTIONS
            .iter()
            .find(|a| a.0 == action)
            .map(|a| a.2.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default();
        match self.cfg.keys.get(action) {
            Some(v) if *v != default => self.key_short(action),
            _ => String::new(),
        }
    }

    pub fn key_short(&self, action: &str) -> String {
        use egui::Key::*;
        match self.keys_for(action).first() {
            None => String::new(),
            Some(Backspace) => "⌫".into(),
            Some(Delete) => "⌦".into(),
            Some(Space) => "␣".into(),
            Some(Enter) => "↵".into(),
            Some(k) => key_name(*k),
        }
    }

    /// Taste für eine Aktion setzen (Platz 0 oder 1); eine doppelte Belegung wird entfernt.
    pub fn bind(&mut self, action: &str, slot: usize, key: Option<egui::Key>) {
        let mut cur: Vec<Option<egui::Key>> = self.keys_for(action).into_iter().map(Some).collect();
        cur.resize(2, None);
        cur[slot] = key;
        if let Some(k) = key {
            for (id, _, _) in ACTIONS {
                if *id != action {
                    let others = self.keys_for(id);
                    if others.contains(&k) {
                        let rest: Vec<String> = others
                            .iter()
                            .filter(|o| **o != k)
                            .map(|o| o.name().to_string())
                            .collect();
                        self.cfg.keys.insert(id.to_string(), rest);
                        let label = ACTIONS
                            .iter()
                            .find(|a| a.0 == *id)
                            .map(|a| a.1)
                            .unwrap_or("");
                        self.toast(format!(
                            "„{}“ war schon bei „{label}“ belegt und wurde dort entfernt.",
                            key_name(k)
                        ));
                    }
                }
            }
        }
        let names: Vec<String> = cur
            .into_iter()
            .flatten()
            .map(|k| k.name().to_string())
            .collect();
        self.cfg.keys.insert(action.to_string(), names);
    }

    pub fn modal_open(&self) -> bool {
        self.dialog.is_some() || self.intro_step.is_some() || self.viewer.is_some()
    }

    // ------------------------------------------------------------------ Großansicht
}
