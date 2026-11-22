use bevy::{prelude::*, window::WindowResizeConstraints};
use bevy_egui::{
    EguiContexts, EguiPlugin, EguiPrimaryContextPass,
    egui::{self, Align, Align2, Color32, FontId, Frame, Margin, RichText, Stroke, pos2, vec2},
};
use projects::Project;

use crate::{
    icons::{self, Icon},
    project_picker::{self, Picker, primary_button},
    recent_projects::{History, RecentProject},
    theme::{self, CANVAS, DEEP, ISLAND, RUST, RUST_HOVER, SECONDARY, SURFACE, TEXT},
};

pub(crate) fn run() -> AppExit {
    App::new()
        .insert_resource(ClearColor(Color::srgb_u8(24, 22, 21)))
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Projetos — Bevy Editor".into(),
                    resolution: (920, 570).into(),
                    resize_constraints: WindowResizeConstraints {
                        min_width: 680.0,
                        min_height: 440.0,
                        ..default()
                    },
                    ..default()
                }),
                ..default()
            }),
            EguiPlugin::default(),
            icons::MaterialSymbols,
        ))
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2d);
        })
        .add_systems(EguiPrimaryContextPass, draw)
        .run()
}

struct Welcome {
    history: Option<History>,
    projects: Vec<RecentProject>,
    picker: Picker,
    search: String,
    notice: Option<String>,
}

impl Default for Welcome {
    fn default() -> Self {
        let mut state = Self {
            history: None,
            projects: Vec::new(),
            picker: Picker::default(),
            search: String::new(),
            notice: None,
        };
        match History::local() {
            Ok(history) => {
                match history.list() {
                    Ok(projects) => state.projects = projects,
                    Err(error) => state.notice = Some(format!("Histórico indisponível: {error}")),
                }
                state.history = Some(history);
            }
            Err(error) => state.notice = Some(format!("Histórico indisponível: {error}")),
        }
        state
    }
}

fn draw(
    mut contexts: EguiContexts,
    mut welcome: Local<Welcome>,
    mut themed: Local<bool>,
    mut exit: MessageWriter<AppExit>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    if !*themed {
        theme::install(ctx);
        *themed = true;
    }
    if welcome.draw(ctx) {
        exit.write(AppExit::Success);
    }
    Ok(())
}

enum RecentAction {
    Open(usize),
    Forget(usize),
}

impl Welcome {
    fn draw(&mut self, ctx: &egui::Context) -> bool {
        let mut outer = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("welcome"),
            egui::UiBuilder::new()
                .layer_id(egui::LayerId::background())
                .max_rect(ctx.viewport_rect()),
        );
        egui::Panel::left("welcome_sidebar")
            .exact_size(238.0)
            .frame(Frame::new().fill(DEEP).inner_margin(Margin::same(24)))
            .show(&mut outer, |ui| self.sidebar(ui));
        let mut action = None;
        egui::CentralPanel::default()
            .frame(Frame::new().fill(CANVAS).inner_margin(Margin::same(32)))
            .show(&mut outer, |ui| action = self.recent(ui));
        match action {
            Some(RecentAction::Open(index)) => {
                let project = &self.projects[index];
                match Project::open(&project.path).and_then(project_picker::launch) {
                    Ok(_) => return true,
                    Err(error) => self.notice = Some(format!("Não foi possível abrir: {error}")),
                }
            }
            Some(RecentAction::Forget(index)) => {
                if let Some(history) = &self.history {
                    match history.forget(&self.projects[index].path) {
                        Ok(()) => {
                            self.projects.remove(index);
                            self.notice = None;
                        }
                        Err(error) => {
                            self.notice = Some(format!("Não foi possível remover: {error}"));
                        }
                    }
                }
            }
            None => {}
        }
        self.picker.window(ctx)
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        brand_art(ui);
        ui.add_space(18.0);
        ui.label(
            RichText::new("BEVY  /  WORKSPACE")
                .size(10.0)
                .color(RUST_HOVER)
                .strong(),
        );
        ui.label(RichText::new("Bevy Editor").size(25.0).strong());
        ui.label(theme::label("Seu espaço para criar jogos."));
        ui.add_space(24.0);
        let width = ui.available_width();
        if ui
            .add_sized([width, 38.0], primary_button("+  Novo projeto"))
            .clicked()
        {
            self.picker.choose(true);
        }
        ui.add_space(5.0);
        if ui
            .add_sized(
                [width, 38.0],
                egui::Button::new(RichText::new("Abrir projeto…").strong()).fill(SURFACE),
            )
            .clicked()
        {
            self.picker.choose(false);
        }
        ui.with_layout(egui::Layout::bottom_up(Align::Min), |ui| {
            ui.label(RichText::new("AMBIENTE LOCAL").size(10.0).color(SECONDARY));
        });
    }

    fn recent(&mut self, ui: &mut egui::Ui) -> Option<RecentAction> {
        ui.label(RichText::new("Projetos recentes").size(30.0).strong());
        ui.label(theme::label("Retome um projeto de onde parou."));
        ui.add_space(18.0);
        ui.add_sized(
            [ui.available_width(), 34.0],
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("Buscar por nome ou caminho…")
                .margin(Margin::symmetric(10, 6)),
        );
        if let Some(notice) = &self.notice {
            ui.label(RichText::new(notice).color(theme::WARNING));
        }
        ui.add_space(14.0);
        if self.projects.is_empty() {
            empty_state(
                ui,
                "Nenhum projeto recente",
                "Crie um projeto ou abra uma pasta para começar.",
            );
            return None;
        }

        let query = self.search.trim().to_lowercase();
        let mut action = None;
        let mut shown = 0;
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for (index, project) in self.projects.iter().enumerate() {
                    let path = project.path.to_string_lossy();
                    if !query.is_empty()
                        && !project.name.to_lowercase().contains(&query)
                        && !path.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    shown += 1;
                    if let Some(next) = ui
                        .push_id(&project.path, |ui| recent_row(ui, index, project, &path))
                        .inner
                    {
                        action = Some(next);
                    }
                    ui.add_space(5.0);
                }
                if shown == 0 {
                    empty_state(ui, "Nenhum resultado", "Tente outro nome ou caminho.");
                }
            });
        action
    }
}

fn recent_row(
    ui: &mut egui::Ui,
    index: usize,
    project: &RecentProject,
    path: &str,
) -> Option<RecentAction> {
    ui.horizontal(|ui| {
        let (rect, row) = ui.allocate_exact_size(
            vec2((ui.available_width() - 38.0).max(1.0), 58.0),
            egui::Sense::click(),
        );
        let row = row
            .on_hover_text(path)
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        row.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &project.name)
        });
        let p = ui.painter();
        p.rect(
            rect,
            theme::CARD_RADIUS,
            if row.hovered() { SURFACE } else { ISLAND },
            Stroke::new(
                1.0,
                if row.has_focus() {
                    RUST
                } else {
                    Color32::TRANSPARENT
                },
            ),
            egui::StrokeKind::Inside,
        );
        let badge = egui::Rect::from_center_size(
            pos2(rect.left() + 29.0, rect.center().y),
            vec2(34.0, 34.0),
        );
        let accents = [
            RUST_HOVER,
            Color32::from_rgb(153, 184, 152),
            Color32::from_rgb(162, 162, 216),
            theme::WARNING,
        ];
        let hash = path
            .bytes()
            .fold(0u8, |acc, byte| acc.wrapping_mul(31).wrapping_add(byte));
        let accent = accents[usize::from(hash) % accents.len()];
        p.rect_filled(badge, 8, accent.gamma_multiply(0.22));
        let initials: String = project
            .name
            .split_whitespace()
            .take(2)
            .filter_map(|word| word.chars().next())
            .flat_map(char::to_uppercase)
            .collect();
        p.text(
            badge.center(),
            Align2::CENTER_CENTER,
            initials,
            FontId::proportional(14.0),
            accent,
        );
        let left = rect.left() + 56.0;
        paint_truncated(
            ui,
            pos2(left, rect.top() + 11.0),
            rect.right() - left - 12.0,
            &project.name,
            14.0,
            TEXT,
        );
        paint_truncated(
            ui,
            pos2(left, rect.top() + 31.0),
            rect.right() - left - 12.0,
            path,
            11.0,
            SECONDARY,
        );

        if icons::sized_button(
            ui,
            vec2(32.0, 58.0),
            Icon::Close,
            false,
            "Remover dos recentes (não apaga arquivos)",
        )
        .clicked()
        {
            Some(RecentAction::Forget(index))
        } else if row.clicked() {
            Some(RecentAction::Open(index))
        } else {
            None
        }
    })
    .inner
}

fn paint_truncated(
    ui: &egui::Ui,
    pos: egui::Pos2,
    width: f32,
    text: &str,
    size: f32,
    color: Color32,
) {
    let job = egui::text::LayoutJob::simple(
        text.into(),
        FontId::proportional(size),
        color,
        width.max(1.0),
    );
    let mut job = job;
    job.wrap.max_rows = 1;
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    ui.painter().galley(pos, galley, color);
}

fn empty_state(ui: &mut egui::Ui, title: &str, description: &str) {
    Frame::new()
        .fill(DEEP)
        .corner_radius(12)
        .inner_margin(Margin::same(22))
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(36.0, 36.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 8, SURFACE);
                icons::paint(ui.painter(), rect.center(), Icon::Folder, RUST_HOVER);
                ui.add_space(6.0);
                ui.label(RichText::new(title).size(15.0).strong());
                ui.label(theme::label(description));
            });
        });
}

fn brand_art(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 120.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 12, DEEP);
    for x in 1..12 {
        for y in 1..7 {
            p.circle_filled(
                rect.left_top() + vec2(x as f32 * rect.width() / 12.0, y as f32 * 16.0 + 4.0),
                1.0,
                SURFACE,
            );
        }
    }
    let c = rect.center();
    let top = c + vec2(0.0, -38.0);
    let left = c + vec2(-34.0, -18.0);
    let right = c + vec2(34.0, -18.0);
    let middle = c + vec2(0.0, 1.0);
    let low_left = c + vec2(-34.0, 20.0);
    let low_right = c + vec2(34.0, 20.0);
    let bottom = c + vec2(0.0, 40.0);
    for (face, color) in [
        ([top, right, middle, left], Color32::from_rgb(90, 49, 32)),
        (
            [left, middle, bottom, low_left],
            Color32::from_rgb(57, 37, 29),
        ),
        (
            [middle, right, low_right, bottom],
            Color32::from_rgb(114, 58, 34),
        ),
    ] {
        p.add(egui::Shape::convex_polygon(
            face.to_vec(),
            color,
            Stroke::new(1.5, RUST),
        ));
    }
    p.circle_filled(top, 2.6, RUST_HOVER);
    p.circle_filled(bottom, 2.6, RUST_HOVER);
}
