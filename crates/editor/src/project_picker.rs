use std::{
    env, io,
    path::{Component, Path},
    process::{Child, Command},
};

use bevy_egui::egui::{self, RichText, vec2};
use projects::Project;

use crate::theme;

#[derive(Default)]
pub(crate) struct Picker {
    visible: bool,
    create: bool,
    folder: String,
    name: String,
    error: Option<String>,
    editors: Vec<Child>,
}

impl Picker {
    pub(crate) fn choose(&mut self, create: bool) {
        self.create = create;
        self.error = None;
        self.visible = true;
    }

    pub(crate) fn window(&mut self, ctx: &egui::Context) -> bool {
        self.editors
            .retain_mut(|child| matches!(child.try_wait(), Ok(None)));
        if !self.visible {
            return false;
        }
        let response = egui::Modal::new(egui::Id::new("project_form"))
            .frame(
                egui::Frame::window(&ctx.style_of(egui::Theme::Dark))
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::same(24)),
            )
            .show(ctx, |ui| self.form(ui));
        let opened = response.inner.is_some();
        self.visible = !opened && self.visible && !response.should_close();
        self.editors.extend(response.inner);
        opened
    }

    fn form(&mut self, ui: &mut egui::Ui) -> Option<Child> {
        ui.set_width(410.0);
        ui.heading(RichText::new("Projetos").size(22.0).strong());
        ui.add_space(6.0);
        let before = self.create;
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.create, false, "Abrir projeto");
            ui.selectable_value(&mut self.create, true, "Novo projeto");
        });
        if before != self.create {
            self.error = None;
        }
        ui.add_space(12.0);
        let mut submit = false;
        if self.create {
            ui.label(theme::section("Nome do projeto"));
            submit |= field(ui, &mut self.name, "Meu jogo");
            ui.add_space(8.0);
        }
        ui.label(theme::section("Pasta do projeto"));
        if self.create {
            let can_browse = folder_name(&self.name).is_some();
            let (enter, browse) = ui
                .horizontal(|ui| {
                    let browse = ui
                        .add_enabled(can_browse, egui::Button::new("Procurar"))
                        .on_disabled_hover_text("Informe um nome válido para a nova pasta.");
                    let enter = field(ui, &mut self.folder, "/caminho/para/meu-jogo");
                    (enter, browse.clicked())
                })
                .inner;
            if browse {
                self.browse_for_destination();
                submit = false;
            } else {
                submit |= enter;
            }
        } else {
            submit |= field(ui, &mut self.folder, "/caminho/para/meu-jogo");
        }
        ui.label(theme::label(if self.create {
            "Escolha onde criar a nova pasta ou digite o caminho completo."
        } else {
            "Informe a pasta que contém project.ron."
        }));
        ui.add_space(14.0);
        let button = if self.create {
            "Criar e abrir"
        } else {
            "Abrir"
        };
        let ready =
            !self.folder.trim().is_empty() && (!self.create || !self.name.trim().is_empty());
        ui.horizontal(|ui| {
            submit |= ui.add_enabled(ready, primary_button(button)).clicked();
            if ui.button("Cancelar").clicked() {
                self.visible = false;
            }
        });
        if ready && submit {
            let root = Path::new(self.folder.trim());
            let project = if self.create {
                Project::create(root, &self.name).inspect(|_| self.create = false)
            } else {
                Project::open(root)
            };
            match project.and_then(launch) {
                Ok(editor) => {
                    self.error = None;
                    return Some(editor);
                }
                Err(error) => self.error = Some(error.to_string()),
            }
        }
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        None
    }

    fn browse_for_destination(&mut self) {
        let Some(name) = folder_name(&self.name) else {
            return;
        };
        let mut dialog = rfd::FileDialog::new().set_title("Escolha onde criar o projeto");
        if let Some(parent) = Path::new(self.folder.trim())
            .parent()
            .filter(|path| path.is_dir())
        {
            dialog = dialog.set_directory(parent);
        }
        if let Some(parent) = dialog.pick_folder() {
            let root = parent.join(name);
            if let Some(path) = root.to_str() {
                self.folder = path.into();
                self.error = None;
            } else {
                self.error = Some("O caminho selecionado contém caracteres inválidos.".into());
            }
        }
    }
}

fn folder_name(name: &str) -> Option<&str> {
    let name = name.trim();
    if name.contains(['/', '\\']) || name.chars().any(char::is_control) {
        return None;
    }
    let mut components = Path::new(name).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Some(name),
        _ => None,
    }
}

fn field(ui: &mut egui::Ui, value: &mut String, hint: &str) -> bool {
    ui.add_sized(
        [ui.available_width(), 32.0],
        egui::TextEdit::singleline(value).hint_text(hint),
    )
    .lost_focus()
        && ui.input(|input| input.key_pressed(egui::Key::Enter))
}

pub(crate) fn primary_button(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).strong().color(theme::CANVAS))
        .fill(theme::RUST_HOVER)
        .min_size(vec2(0.0, 34.0))
}

// Asset sources are set before Bevy starts; open each project in its own editor process.
pub(crate) fn launch(project: Project) -> io::Result<Child> {
    Command::new(env::current_exe()?)
        .arg("open")
        .arg(project.root())
        .spawn()
}

#[cfg(test)]
mod tests {
    use super::folder_name;

    #[test]
    fn project_folder_is_a_single_path_component() {
        assert_eq!(folder_name(" Meu jogo "), Some("Meu jogo"));
        for name in ["", ".", "..", "../jogo", "/jogo", "a/b", "a\\b", "a\0b"] {
            assert_eq!(folder_name(name), None, "{name:?}");
        }
    }
}
