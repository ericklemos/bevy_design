use std::{
    collections::BTreeMap,
    fs,
    io::{self, Write},
    path::PathBuf,
};

use bevy_egui::egui::{Pos2, pos2};

pub(super) struct Layout {
    path: PathBuf,
    positions: BTreeMap<String, [f32; 2]>,
    dirty: bool,
    pub(super) status: Option<String>,
}

impl Layout {
    pub(super) fn load(path: PathBuf) -> Self {
        let result = match fs::read_to_string(&path) {
            Ok(raw) => ron::from_str::<BTreeMap<String, [f32; 2]>>(&raw).map_err(io::Error::other),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(error) => Err(error),
        };
        let (positions, status) = match result {
            Ok(mut positions) => {
                positions.retain(|_, [x, y]| {
                    x.is_finite() && y.is_finite() && x.abs() < 1e6 && y.abs() < 1e6
                });
                (positions, None)
            }
            Err(error) => (
                BTreeMap::new(),
                Some(format!("Não foi possível ler o layout: {error}")),
            ),
        };
        Self {
            path,
            positions,
            dirty: false,
            status,
        }
    }

    pub(super) fn get(&self, key: &str) -> Option<Pos2> {
        self.positions.get(key).map(|&[x, y]| pos2(x, y))
    }

    pub(super) fn set(&mut self, key: String, pos: Pos2) {
        self.positions.insert(key, [pos.x, pos.y]);
        self.dirty = true;
    }

    pub(super) fn reset(&mut self, prefix: &str) {
        let before = self.positions.len();
        self.positions.retain(|key, _| !key.starts_with(prefix));
        self.dirty |= before != self.positions.len();
    }

    pub(super) fn save_if_dirty(&mut self) {
        if self.dirty {
            self.dirty = false;
            self.status = self
                .save()
                .err()
                .map(|error| format!("Não foi possível salvar o layout: {error}"));
        }
    }

    fn save(&self) -> io::Result<()> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| io::Error::other("layout sem diretório"))?;
        fs::create_dir_all(parent)?;
        let ron = ron::ser::to_string_pretty(&self.positions, ron::ser::PrettyConfig::default())
            .map_err(io::Error::other)?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        writeln!(temp, "{ron}")?;
        temp.persist(&self.path).map_err(|error| error.error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_positions_roundtrip_and_reset_by_view() {
        let project = tempfile::tempdir().unwrap();
        let path = project.path().join(".editor/graph_layout.ron");
        let mut layout = Layout::load(path.clone());
        layout.set("order/Update/node".into(), pos2(31.25, -20.0));
        layout.set("observer/node".into(), pos2(100.0, 200.0));
        layout.save_if_dirty();
        assert!(layout.status.is_none());
        let mut layout = Layout::load(path.clone());
        assert_eq!(layout.get("order/Update/node"), Some(pos2(31.25, -20.0)));
        layout.reset("order/Update/");
        layout.save_if_dirty();
        let layout = Layout::load(path);
        assert_eq!(layout.get("order/Update/node"), None);
        assert_eq!(layout.get("observer/node"), Some(pos2(100.0, 200.0)));
    }
}
