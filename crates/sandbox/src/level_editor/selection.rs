//! Selection, clipboard and joining actions all commit as one undo operation.
use super::*;

impl LevelEditor {
    pub(super) fn selection_center(&self) -> sim::Vec2 {
        if self.selection.len() < 2 {
            return self
                .selected
                .map_or(sim::Vec2::ZERO, |i| self.level.items[i].position());
        }
        let (lo, hi) = self
            .selection
            .iter()
            .map(|&i| item_bounds(&self.level.items[i]))
            .fold(
                (
                    sim::Vec2::splat(f32::INFINITY),
                    sim::Vec2::splat(f32::NEG_INFINITY),
                ),
                |(lo, hi), (a, b)| (lo.min(a), hi.max(b)),
            );
        (lo + hi) * 0.5
    }
    pub(super) fn clear_selection(&mut self) {
        self.selection.clear();
        self.selected = None;
    }
    pub(super) fn select_one(&mut self, i: usize) {
        self.selection = vec![i];
        self.selected = Some(i);
    }
    pub(super) fn copy_selection(&mut self) {
        self.clipboard = self
            .selection
            .iter()
            .map(|&i| self.level.items[i].clone())
            .collect();
        self.paste_count = 0;
        self.status = format!(
            "Copied {} items. Ctrl V pastes; Ctrl D duplicates.",
            self.clipboard.len()
        );
    }
    pub(super) fn paste(&mut self) {
        let mut copies: Vec<_> = self
            .clipboard
            .iter()
            .filter(|item| {
                !self.level.items.iter().any(|old| {
                    matches!((item,old),
                (LevelItem::Spawn {team:a,..},LevelItem::Spawn {team:b,..}) |
                (LevelItem::Flag {team:a,..},LevelItem::Flag {team:b,..}) if a==b)
                })
            })
            .cloned()
            .collect();
        if copies.is_empty() {
            self.status = "Copy objects first. Each team keeps one spawn and one flag.".into();
            return;
        }
        if self.level.items.len() + copies.len() > sim::level::MAX_LEVEL_OBJECTS {
            self.status = "The map's object budget is full; terrain does not count.".into();
            return;
        }
        self.paste_count += 1;
        let ids: Vec<_> = (0..copies.len()).collect();
        // A clipboard may outlive the map it came from. Fit the group uniformly
        // to a smaller map before applying its shared placement offset.
        if let Err(e) = fit_clipboard(&mut copies, self.level.size.into()) {
            self.status = e;
            return;
        }
        let delta = sim::level::clamp_translation(
            &copies,
            &ids,
            self.level.size.into(),
            sim::Vec2::splat(self.paste_count as f32),
        );
        for copy in &mut copies {
            copy.translate(delta);
        }
        let before = self.level.clone();
        self.selection = (self.level.items.len()..self.level.items.len() + copies.len()).collect();
        self.level.items.extend(copies);
        self.clamp_selection();
        self.tool = Tool::Select;
        self.finish_change(before);
    }
    pub(super) fn delete(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let before = self.level.clone();
        self.selection.sort_unstable();
        for &i in self.selection.iter().rev() {
            self.level.items.remove(i);
        }
        self.clear_selection();
        self.finish_change(before);
    }
    fn join_indices(&mut self, ids: &[usize]) -> Result<(), String> {
        let items: Vec<_> = ids.iter().map(|&i| self.level.items[i].clone()).collect();
        let merged = sim::level::merge_items(&items)?;
        let mut ids = ids.to_vec();
        ids.sort_unstable();
        let index = ids[0];
        for &i in ids.iter().skip(1).rev() {
            self.level.items.remove(i);
        }
        self.level.items[index] = merged;
        self.select_one(index);
        Ok(())
    }
    pub(super) fn merge_selection(&mut self) {
        self.commit_field();
        let before = self.level.clone();
        match self.join_indices(&self.selection.clone()) {
            Ok(()) => {
                self.finish_change(before);
                self.status = "Merged into one shape and one fracture mesh.".into();
            }
            Err(e) => self.status = e,
        }
    }
    pub(super) fn join_touching(&mut self) {
        if !self.join || self.selection.len() != 1 {
            return;
        }
        loop {
            let i = self.selection[0];
            let a = &self.level.items[i];
            let (lo, hi) = item_bounds(a);
            let candidates: Vec<_> = self
                .level
                .items
                .iter()
                .enumerate()
                .filter(|(j, b)| {
                    let (blo, bhi) = item_bounds(b);
                    *j != i && a.can_join(b) && lo.cmple(bhi).all() && hi.cmpge(blo).all()
                })
                .map(|(j, _)| j)
                .collect();
            let mut changed = false;
            for j in candidates {
                if self.join_indices(&[i, j]).is_ok() {
                    changed = true;
                    break;
                }
            }
            if !changed {
                break;
            }
        }
    }
    /// Magnetic edge alignment applies only to compatible walls, never barrels.
    /// Alt temporarily disables both magnets and automatic joining.
    pub(super) fn sticky_delta(&self, desired: sim::Vec2) -> sim::Vec2 {
        if !self.join
            || self.selection.len() != 1
            || is_key_down(KeyCode::LeftAlt)
            || is_key_down(KeyCode::RightAlt)
        {
            return desired;
        }
        let i = self.selection[0];
        let a = &self.level.items[i];
        if a.angle().sin().abs().min(a.angle().cos().abs()) > 0.01 {
            return desired;
        }
        if !matches!(a, LevelItem::Solid { .. })
            && !matches!(a,LevelItem::Object{asset,..} if asset.surface == sim::Surface::Masonry && !asset.is_circular())
        {
            return desired;
        }
        let (lo, hi) = item_bounds(a);
        let (lo, hi) = (lo + desired, hi + desired);
        let mut snap = sim::Vec2::splat(0.25);
        let mut delta = sim::Vec2::ZERO;
        for (j, b) in self.level.items.iter().enumerate() {
            if j == i || !a.can_join(b) || b.angle().sin().abs().min(b.angle().cos().abs()) > 0.01 {
                continue;
            }
            let (blo, bhi) = item_bounds(b);
            if lo.y <= bhi.y + 0.2 && hi.y >= blo.y - 0.2 {
                for d in [blo.x - hi.x, bhi.x - lo.x] {
                    if d.abs() < snap.x {
                        snap.x = d.abs();
                        delta.x = d;
                    }
                }
            }
            if lo.x <= bhi.x + 0.2 && hi.x >= blo.x - 0.2 {
                for d in [blo.y - hi.y, bhi.y - lo.y] {
                    if d.abs() < snap.y {
                        snap.y = d.abs();
                        delta.y = d;
                    }
                }
            }
        }
        desired + delta
    }
}

fn fit_clipboard(items: &mut [LevelItem], size: sim::Vec2) -> Result<(), String> {
    let bounds = |items: &[LevelItem]| {
        items.iter().map(item_bounds).fold(
            (
                sim::Vec2::splat(f32::INFINITY),
                sim::Vec2::splat(f32::NEG_INFINITY),
            ),
            |(lo, hi), (a, b)| (lo.min(a), hi.max(b)),
        )
    };
    let half = size * 0.5 - sim::Vec2::splat(0.201);
    for _ in 0..8 {
        let (lo, hi) = bounds(items);
        let center = (lo + hi) * 0.5;
        let scale = (half * 2.0 / (hi - lo)).min_element().min(1.0);
        if scale >= 0.999999 {
            break;
        }
        let scale = scale * 0.9999;
        for item in items.iter_mut() {
            let pos = item.position();
            if !matches!(item, LevelItem::Flag { .. } | LevelItem::Spawn { .. }) {
                item.resize(item.dimensions() * scale)?;
            }
            item.translate((pos - center) * scale - pos);
        }
    }
    let (lo, hi) = bounds(items);
    if (hi - lo).cmpgt(half * 2.0 + sim::Vec2::splat(0.0001)).any() {
        return Err("The copied group cannot fit on this map.".into());
    }
    let delta = (-half - lo).max(sim::Vec2::ZERO) + (half - hi).min(sim::Vec2::ZERO);
    for item in items {
        item.translate(delta);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_join_closes_a_room_and_connects_a_divider_at_both_ends() {
        let mut editor = LevelEditor::new();
        editor.level.items.clear();
        for (x, y, width, height) in [
            (0., -3., 8., 2.),
            (-3., 0., 2., 8.),
            (0., 3., 8., 2.),
            (3., 0., 2., 8.),
            (0., 0., 2., 4.),
        ] {
            editor.level.items.push(LevelItem::Object {
                asset: Box::new(sim::level::wall_asset(sim::Vec2::new(width, height))),
                position: [x, y],
                angle: 0.,
                cutouts: vec![],
            });
            editor.select_one(editor.level.items.len() - 1);
            editor.join_touching();
            assert_eq!(
                editor.level.items.len(),
                1,
                "every drawn section must connect"
            );
        }
        let mesh = editor.level.resolved_meshes().unwrap().remove(0).unwrap();
        let center = editor.level.items[0].position();
        assert!((mesh.area - 56.0).abs() < 0.01);
        assert!(!mesh.contains_point(sim::Vec2::new(-1.5, 0.) - center));
        assert!(!mesh.contains_point(sim::Vec2::new(1.5, 0.) - center));
        assert!(mesh.contains_point(-center));
        assert_eq!(
            editor.level,
            Level::from_json(&editor.level.to_json().unwrap()).unwrap()
        );
    }
}

pub(super) fn item_bounds(item: &LevelItem) -> (sim::Vec2, sim::Vec2) {
    item.world_outline().into_iter().fold(
        (
            sim::Vec2::splat(f32::INFINITY),
            sim::Vec2::splat(f32::NEG_INFINITY),
        ),
        |(lo, hi), p| (lo.min(p), hi.max(p)),
    )
}
