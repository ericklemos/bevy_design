use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use bevy::prelude::Resource;
use bevy_egui::egui::{self, Align2, FontId, Rect, Sense, Stroke, pos2, vec2};

use crate::{icons::Icon, theme};

#[derive(Resource)]
pub(crate) struct Index {
    root: PathBuf,
    folders: BTreeMap<PathBuf, Vec<PathBuf>>,
}

impl Index {
    pub(crate) fn new(root: PathBuf) -> Self {
        let mut index = Self {
            root,
            folders: BTreeMap::new(),
        };
        index.refresh();
        index
    }

    fn refresh(&mut self) {
        self.folders.clear();
        let mut all = Vec::new();
        let mut pending = vec![PathBuf::from("scenes"), PathBuf::from("assets")];
        while let Some(folder) = pending.pop() {
            let Ok(entries) = fs::read_dir(self.root.join(&folder)) else {
                continue;
            };
            let mut files = Vec::new();
            for entry in entries.flatten() {
                let name = entry.file_name();
                if name.to_string_lossy().starts_with('.') {
                    continue;
                }
                let path = folder.join(name);
                if let Ok(kind) = entry.file_type() {
                    if kind.is_dir() {
                        pending.push(path);
                    } else if kind.is_file() {
                        files.push(path);
                    }
                }
            }
            files.sort();
            all.extend(files.iter().cloned());
            self.folders.insert(folder, files);
        }
        all.sort();
        self.folders.insert(PathBuf::new(), all);
    }
}

#[derive(Default)]
pub(crate) struct Browser {
    folder: PathBuf,
    selected: Option<PathBuf>,
}

impl Browser {
    pub(crate) fn draw(&mut self, ui: &mut egui::Ui, index: &Index) -> Option<PathBuf> {
        let mut selection = None;
        ui.horizontal_top(|ui| {
            let width = (ui.available_width() * 0.34).clamp(66.0, 120.0);
            ui.allocate_ui_with_layout(
                vec2(width, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(width);
                    ui.label(theme::label("FOLDERS").small());
                    for path in index.folders.keys() {
                        let indent =
                            path.components().count().saturating_sub(1).min(3) as f32 * 8.0;
                        ui.horizontal(|ui| {
                            ui.add_space(indent);
                            let title = path
                                .file_name()
                                .map_or("Project".into(), |s| s.to_string_lossy());
                            if theme::row(ui, self.folder == *path, title.as_ref(), Icon::Folder)
                                .on_hover_text(path.display().to_string())
                                .clicked()
                            {
                                self.folder.clone_from(path);
                            }
                        });
                    }
                },
            );
            ui.add_space(8.0);
            ui.vertical(|ui| {
                let files = index
                    .folders
                    .get(&self.folder)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                ui.label(theme::label(format!("{} FILES", files.len())).small());
                if files.is_empty() {
                    ui.label(theme::label("Pasta vazia"));
                }
                let width = ui.available_width().clamp(42.0, 76.0);
                ui.horizontal_wrapped(|ui| {
                    for path in files {
                        ui.push_id(path, |ui| {
                            if card(ui, path, width, self.selected.as_ref() == Some(path)).clicked()
                            {
                                self.selected = Some(path.clone());
                                selection = Some(path.clone());
                            }
                        });
                    }
                });
            });
        });
        selection
    }

    pub(crate) fn selected(&self) -> Option<&Path> {
        self.selected.as_deref()
    }

    pub(crate) fn clear_selection(&mut self) {
        self.selected = None;
    }
}

fn card(ui: &mut egui::Ui, path: &Path, width: f32, selected: bool) -> egui::Response {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_uppercase();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 84.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, &name)
    });
    let p = ui.painter();
    let preview = Rect::from_min_size(rect.min, vec2(width, 62.0));
    p.rect_filled(
        preview,
        theme::CARD_RADIUS,
        if response.hovered() {
            theme::HOVER
        } else if selected {
            theme::SURFACE
        } else {
            theme::ISLAND
        },
    );
    let doc = Rect::from_center_size(preview.center() - vec2(0.0, 6.0), vec2(19.0, 24.0));
    let stroke = Stroke::new(1.5, theme::SECONDARY);
    p.rect_stroke(doc, 3, stroke, egui::StrokeKind::Inside);
    for y in [8.0, 13.0, 18.0] {
        p.line_segment([doc.min + vec2(4.0, y), doc.min + vec2(14.0, y)], stroke);
    }
    p.text(
        preview.center_bottom() - vec2(0.0, 4.0),
        Align2::CENTER_BOTTOM,
        ext,
        FontId::monospace(9.0),
        theme::SECONDARY,
    );
    let galley = egui::WidgetText::from(egui::RichText::new(name.as_ref()).color(if selected {
        theme::RUST_HOVER
    } else {
        theme::TEXT
    }))
    .into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        width,
        FontId::proportional(11.0),
    );
    p.galley(
        pos2(
            rect.center().x - galley.size().x * 0.5,
            preview.bottom() + 4.0,
        ),
        galley,
        theme::TEXT,
    );
    response.on_hover_text(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_project_assets_and_scenes_by_folder() {
        let project = tempfile::tempdir().unwrap();
        fs::create_dir_all(project.path().join("assets/configs")).unwrap();
        fs::create_dir(project.path().join("scenes")).unwrap();
        for path in [
            "assets/configs/player.ron",
            "scenes/level.scn.ron",
            "unrelated.txt",
        ] {
            fs::write(project.path().join(path), "").unwrap();
        }
        let index = Index::new(project.path().to_owned());
        assert_eq!(index.folders[Path::new("")].len(), 2);
        assert_eq!(
            index.folders[Path::new("assets/configs")],
            [PathBuf::from("assets/configs/player.ron")]
        );
    }
}
