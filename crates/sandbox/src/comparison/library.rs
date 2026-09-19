//! Saved recipes, checkpoints and experiment library UI.
use super::*;

impl Comparison {
    pub(super) fn load(&mut self, experiment: Experiment) {
        self.selected = experiment.favorite.unwrap_or(0);
        self.cursor = experiment.cursor_tick as f32;
        self.message = if experiment.engine != ENGINE_REVISION {
            format!(
                "Saved frame from {}. RUN replays with {}.",
                experiment.engine, ENGINE_REVISION
            )
        } else {
            "Saved comparison restored. RUN reproduces its full timeline.".into()
        };
        self.experiment = experiment;
        self.focus = false;
        self.fit_view();
        self.input = None;
        self.error = None;
        self.runner = None;
        self.calculating = false;
        self.playing = false;
        self.library_open = false;
        if self.experiment.review.is_empty() {
            self.prepare();
        }
    }
    pub(super) fn checkpoint(&self) -> Experiment {
        let mut e = self.experiment.clone();
        if let Some(r) = &self.runner {
            let tick = (self.cursor as u32).min(r.completed_tick);
            e.review = r.recordings.iter().map(|r| r.state(tick)).collect();
            e.cursor_tick = e.review[0].frame.tick;
        }
        e
    }
    pub(super) fn save(&mut self) {
        if let Some(e) = &self.library_error {
            self.message = format!("Library unavailable: {e} Export a JSON copy.");
            return;
        }
        let e = self.checkpoint();
        let mut library = self.archive.clone();
        if let Some(i) = library
            .experiments
            .iter()
            .position(|old| old.name == e.name)
        {
            library.experiments[i] = e;
        } else {
            library.experiments.push(e);
        }
        match library.to_json().and_then(|json| storage::persist(&json)) {
            Ok(()) => {
                self.archive = library;
                self.message =
                    "Saved recipe, favorite and the displayed frame. Export for a portable copy."
                        .into();
            }
            Err(e) => self.message = e,
        }
    }
    pub(super) fn export(&mut self) {
        self.message = match self
            .checkpoint()
            .to_json()
            .and_then(|s| storage::export(&s))
        {
            Ok(s) => s.into(),
            Err(e) => e,
        };
    }
    pub(super) fn library(&mut self, size: Vec2, mouse: Vec2) {
        draw_rectangle(0.0, 0.0, size.x, size.y, Color::new(0.0, 0.0, 0.02, 0.88));
        let x = (size.x - 800.0) / 2.0;
        draw_rectangle(x, 110.0, 800.0, 675.0, PAPER);
        draw_rectangle_lines(x, 110.0, 800.0, 675.0, 2.0, CYAN);
        ui::text("SAVED EXPERIMENTS", x + 24.0, 149.0, 25.0, INK);
        if ui::button(
            Rect::new(x + 655.0, 121.0, 120.0, 37.0),
            "CLOSE",
            mouse,
            false,
        ) || is_key_pressed(KeyCode::Escape)
        {
            self.library_open = false;
            return;
        }
        ui::text(
            "Load a recipe and its saved comparison frame.",
            x + 24.0,
            178.0,
            17.0,
            MUTED,
        );
        if self.archive.experiments.is_empty() {
            ui::text(
                "No experiments saved yet. Use SAVE in the range.",
                x + 24.0,
                238.0,
                18.0,
                MUTED,
            );
        }
        let mut load = None;
        let mut delete = None;
        for (i, e) in self.archive.experiments.iter().enumerate() {
            let y = 200.0 + i as f32 * 43.0;
            if ui::button(
                Rect::new(x + 16.0, y, 650.0, 37.0),
                &format!(
                    "{} / {} CASES / {:.2}s",
                    e.name.chars().take(32).collect::<String>(),
                    e.variants.len(),
                    e.cursor_tick as f32 / 120.0
                ),
                mouse,
                false,
            ) {
                load = Some(i);
            }
            if ui::button(Rect::new(x + 681.0, y, 100.0, 37.0), "REMOVE", mouse, false) {
                delete = Some(i);
            }
        }
        if let Some(i) = load {
            self.load(self.archive.experiments[i].clone());
        }
        if let Some(i) = delete {
            let mut a = self.archive.clone();
            a.experiments.remove(i);
            match a.to_json().and_then(|s| storage::persist(&s)) {
                Ok(()) => self.archive = a,
                Err(e) => self.message = e,
            }
        }
        ui::text(
            "Up to 12 recipes. EXPORT keeps a portable JSON copy.",
            x + 24.0,
            764.0,
            16.0,
            MUTED,
        );
    }
}
