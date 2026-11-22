use bevy::{ecs::system::SystemParam, prelude::*, world_serialization::DynamicWorldRoot};
use bevy_egui::{EguiContexts, egui};
use egui::{Color32, RichText, Ui, vec2};
use egui_dock::{DockArea, DockState, NodeIndex, TabViewer};
use game::Health;
use std::collections::HashSet;

use crate::{
    EditorOnly,
    config::ConfigEditor,
    flow_graph::{FlowGraph, Model},
    gizmo::{self, Gizmo},
    icons::{self, Icon},
    inspector::{Edit, Inspector},
    session::{self, Action, Mode, Session},
    theme::{self, CANVAS, RUST, SECONDARY, SURFACE, TEXT},
    viewport::{EditCamera, FlyCamera, Viewport},
};

#[derive(Clone, Copy, Debug, Hash)]
enum Panel {
    Hierarchy,
    Viewport,
    Graph,
    Inspector,
    Assets,
    Console,
}

fn default_dock() -> DockState<Panel> {
    const LEFT: f32 = 0.20;
    const CENTER: f32 = 0.58;
    const TOP: f32 = 0.58;
    let mut dock = DockState::new(vec![Panel::Viewport]);
    let tree = dock.main_surface_mut();
    // The fraction is the left child's share, even when the new tab is on that side.
    let [center, left] = tree.split_left(NodeIndex::root(), LEFT, vec![Panel::Hierarchy]);
    let [center, _] = tree.split_right(center, CENTER / (1.0 - LEFT), vec![Panel::Inspector]);
    let [_, bottom] = tree.split_below(center, TOP, vec![Panel::Graph]);
    tree.split_right(bottom, 0.57, vec![Panel::Console]);
    tree.split_below(left, TOP, vec![Panel::Assets]);
    dock
}

#[cfg(test)]
mod dock_tests {
    use super::*;
    use egui_dock::Node;

    #[test]
    fn initial_dock_gives_hierarchy_the_narrow_left_column() {
        let dock = default_dock();
        let tree = dock.main_surface();
        let root = NodeIndex::root();
        let Node::Horizontal(split) = &tree[root] else {
            panic!("expected left column split")
        };
        let Node::Horizontal(rest) = &tree[root.right()] else {
            panic!("expected right column split")
        };
        let widths = [
            split.fraction,
            (1.0 - split.fraction) * rest.fraction,
            (1.0 - split.fraction) * (1.0 - rest.fraction),
        ];
        for (actual, expected) in widths.into_iter().zip([0.20, 0.58, 0.22]) {
            assert!((actual - expected).abs() < 1e-5);
        }
        assert!(matches!(
            tree[root.left().left()].tabs(),
            Some([Panel::Hierarchy])
        ));
    }

    #[test]
    fn viewport_graph_and_console_have_separate_panes() {
        let dock = default_dock();
        let tree = dock.main_surface();
        let center = NodeIndex::root().right().left();
        assert!(matches!(
            tree[center.left()].tabs(),
            Some([Panel::Viewport])
        ));
        assert!(matches!(
            tree[center.right().left()].tabs(),
            Some([Panel::Graph])
        ));
        assert!(matches!(
            tree[center.right().right()].tabs(),
            Some([Panel::Console])
        ));
    }
}

#[derive(Resource)]
pub(crate) struct Workspace {
    dock: DockState<Panel>,
    pub(crate) selected: Option<Entity>,
    config_selected: bool,
    search: String,
    hierarchy: Hierarchy,
    assets: crate::asset_browser::Browser,
    console: crate::console::View,
    initialized_selection: bool,
    pub(crate) grid_visible: bool,
    gizmo: Gizmo,
}

#[derive(Default)]
struct Hierarchy {
    sort: bool,
    players_only: bool,
    collapsed: HashSet<Entity>,
    locked: HashSet<Entity>,
    hidden: HashSet<Entity>,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            dock: default_dock(),
            selected: None,
            config_selected: false,
            search: String::new(),
            hierarchy: Hierarchy::default(),
            assets: Default::default(),
            console: Default::default(),
            initialized_selection: false,
            grid_visible: true,
            gizmo: Gizmo::default(),
        }
    }
}

impl Workspace {
    pub(crate) fn preview_visible(&self, entity: Entity) -> bool {
        !self.hierarchy.hidden.contains(&entity)
    }
}

type SceneNodes<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static Name>,
        Option<&'static Children>,
        Option<&'static Health>,
    ),
    Without<EditorOnly>,
>;

struct Row {
    entity: Entity,
    label: String,
    player: bool,
    children: Vec<Row>,
}

impl Row {
    fn read(entity: Entity, nodes: &SceneNodes) -> Option<Self> {
        let (name, children, health) = nodes.get(entity).ok()?;
        Some(Self {
            entity,
            player: health.is_some(),
            label: name
                .map(|name| name.as_str().to_owned())
                .unwrap_or_else(|| {
                    if health.is_some() {
                        "Player".into()
                    } else {
                        format!("Entity {entity}")
                    }
                }),
            children: children
                .into_iter()
                .flat_map(Children::iter)
                .filter_map(|child| Self::read(child, nodes))
                .collect(),
        })
    }

    fn find(&self, entity: Entity) -> Option<&Self> {
        if self.entity == entity {
            Some(self)
        } else {
            self.children.iter().find_map(|row| row.find(entity))
        }
    }

    fn player(&self) -> Option<Entity> {
        self.player
            .then_some(self.entity)
            .or_else(|| self.children.iter().find_map(Self::player))
    }
}

#[derive(SystemParam)]
pub(crate) struct Tools<'w, 's> {
    inspector: Res<'w, Inspector>,
    config: ResMut<'w, ConfigEditor>,
    flow_graph: ResMut<'w, FlowGraph>,
    flow_model: Res<'w, Model>,
    asset_index: Res<'w, crate::asset_browser::Index>,
    logs: Res<'w, crate::console::LogBuffer>,
    transforms: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static GlobalTransform,
            Option<&'static ChildOf>,
        ),
        Without<EditorOnly>,
    >,
    globals: Query<'w, 's, &'static GlobalTransform>,
}

enum EntityAction {
    Create,
    Delete(Entity),
}

pub(crate) fn draw(
    mut contexts: EguiContexts,
    mut workspace: ResMut<Workspace>,
    session: Res<Session>,
    roots: Query<Entity, With<DynamicWorldRoot>>,
    children: Query<&Children>,
    nodes: SceneNodes,
    viewport: Res<Viewport>,
    mut images: ResMut<Assets<Image>>,
    mut fly: ResMut<FlyCamera>,
    mut camera: Single<(&Camera, &mut Transform), (With<EditCamera>, With<EditorOnly>)>,
    mut tools: Tools,
    mut commands: Commands,
    mut themed: Local<bool>,
) -> Result {
    let root = session.restored_root.or_else(|| roots.single().ok());
    let rows: Vec<_> = root
        .and_then(|root| children.get(root).ok())
        .into_iter()
        .flat_map(Children::iter)
        .filter_map(|child| Row::read(child, &nodes))
        .collect();
    let texture = contexts.image_id(&viewport.0);
    let ctx = contexts.ctx_mut()?;
    if !*themed {
        theme::install(ctx);
        *themed = true;
    }
    let mut outer = Ui::new(
        ctx.clone(),
        egui::Id::new("editor"),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );
    egui::Panel::top("menu")
        .exact_size(theme::HEADER)
        .frame(
            egui::Frame::new()
                .fill(CANVAS)
                .inner_margin(egui::Margin::symmetric(10, 2)),
        )
        .show(&mut outer, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Fechar editor").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        commands.write_message(AppExit::Success);
                    }
                });
                ui.menu_button("Editors", |ui| {
                    if ui.button("Restaurar layout inicial").clicked() {
                        workspace.dock = default_dock();
                        ui.close();
                    }
                });
                ui.menu_button("Help", |ui| {
                    ui.label("Viewport: arraste com o botão direito.");
                    ui.label("WASD move; Q/E desce/sobe; roda aproxima.");
                });
            });
        });

    let Workspace {
        dock,
        selected,
        search,
        config_selected,
        gizmo,
        hierarchy,
        assets,
        console,
        initialized_selection,
        grid_visible,
    } = &mut *workspace;
    if selected.is_some_and(|entity| rows.iter().all(|row| row.find(entity).is_none())) {
        *selected = None;
    }
    if !*initialized_selection && !rows.is_empty() {
        *initialized_selection = true;
        if !*config_selected {
            *selected = rows.iter().find_map(Row::player).or(Some(rows[0].entity));
        }
    }
    for flags in [
        &mut hierarchy.hidden,
        &mut hierarchy.locked,
        &mut hierarchy.collapsed,
    ] {
        flags.retain(|entity| rows.iter().any(|row| row.find(*entity).is_some()));
    }
    let target = selected_target(*selected, &tools.transforms, &tools.globals);
    let mut viewer = Viewer {
        session: &session,
        rows: &rows,
        selected,
        search,
        hierarchy,
        assets,
        asset_index: &tools.asset_index,
        console,
        logs: &tools.logs,
        config_selected,
        config: &mut tools.config,
        flow_graph: &mut tools.flow_graph,
        flow_model: &tools.flow_model,
        inspector: &tools.inspector,
        edits: Vec::new(),
        root_present: root.is_some(),
        entity_action: None,
        texture,
        fly: &mut fly,
        camera: camera.0,
        gizmo,
        grid_visible,
        target,
        transformed: None,
        view_size: None,
        action: None,
    };
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(CANVAS)
                .inner_margin(egui::Margin::same(theme::GUTTER)),
        )
        .show(&mut outer, |ui| {
            let style = theme::dock(ui.style());
            DockArea::new(dock)
                .style(style)
                .show_inside(ui, &mut viewer);
        });
    if let Some(size) = viewer.view_size {
        viewport.resize(&mut images, size, ctx.pixels_per_point());
    }
    for edit in viewer.edits {
        commands.queue(move |world: &mut World| edit.apply(world));
    }
    if let Some((entity, transform)) = viewer.transformed {
        commands.queue(move |world: &mut World| {
            if let Some(mut current) = world.get_mut::<Transform>(entity)
                && *current != transform
            {
                *current = transform;
            }
        });
    }
    if let (Some(root), Some(action)) = (root, viewer.entity_action) {
        match action {
            EntityAction::Create => {
                *viewer.selected = Some(
                    commands
                        .spawn((
                            Name::new("Nova entidade"),
                            Transform::default(),
                            ChildOf(root),
                        ))
                        .id(),
                );
            }
            EntityAction::Delete(entity) => {
                commands.entity(entity).try_despawn();
                *viewer.selected = None;
            }
        }
    }
    if let (Some(root), Some(action)) = (root, viewer.action) {
        commands.queue(move |world: &mut World| session::transition(world, root, action));
    }
    if *camera.1 != fly.transform {
        *camera.1 = fly.transform;
    }
    Ok(())
}

fn selected_target(
    selected: Option<Entity>,
    transforms: &Query<(&Transform, &GlobalTransform, Option<&ChildOf>), Without<EditorOnly>>,
    globals: &Query<&GlobalTransform>,
) -> Option<gizmo::Target> {
    let entity = selected?;
    let (local, global, parent) = transforms.get(entity).ok()?;
    Some(gizmo::Target {
        entity,
        local: *local,
        global: *global,
        parent: parent
            .and_then(|parent| globals.get(parent.parent()).ok())
            .copied(),
    })
}

struct Viewer<'a> {
    session: &'a Session,
    rows: &'a [Row],
    selected: &'a mut Option<Entity>,
    search: &'a mut String,
    hierarchy: &'a mut Hierarchy,
    assets: &'a mut crate::asset_browser::Browser,
    asset_index: &'a crate::asset_browser::Index,
    console: &'a mut crate::console::View,
    logs: &'a crate::console::LogBuffer,
    config_selected: &'a mut bool,
    config: &'a mut ConfigEditor,
    flow_graph: &'a mut FlowGraph,
    flow_model: &'a Model,
    inspector: &'a Inspector,
    edits: Vec<Edit>,
    root_present: bool,
    entity_action: Option<EntityAction>,
    texture: Option<egui::TextureId>,
    fly: &'a mut FlyCamera,
    camera: &'a Camera,
    gizmo: &'a mut Gizmo,
    grid_visible: &'a mut bool,
    target: Option<gizmo::Target>,
    transformed: Option<(Entity, Transform)>,
    view_size: Option<egui::Vec2>,
    action: Option<Action>,
}

impl TabViewer for Viewer<'_> {
    type Tab = Panel;

    fn id(&mut self, tab: &mut Panel) -> egui::Id {
        egui::Id::new(*tab)
    }

    fn title(&mut self, tab: &mut Panel) -> egui::WidgetText {
        let mut job = egui::text::LayoutJob::default();
        job.append(
            match tab {
                Panel::Hierarchy => "Hierarchy",
                Panel::Viewport => "Viewport",
                Panel::Graph => "Flow Graph",
                Panel::Inspector => "Inspector",
                Panel::Assets => "Asset Browser",
                Panel::Console => "Console",
            },
            18.0,
            egui::TextFormat::simple(egui::FontId::proportional(12.0), TEXT),
        );
        job.into()
    }

    fn on_tab_button(&mut self, tab: &mut Panel, response: &egui::Response) {
        let glyph = match tab {
            Panel::Hierarchy => Icon::Tree,
            Panel::Viewport => Icon::View,
            Panel::Graph => Icon::Graph,
            Panel::Inspector => Icon::Inspector,
            Panel::Assets => Icon::Folder,
            Panel::Console => Icon::Terminal,
        };
        icons::paint(
            &response
                .ctx
                .layer_painter(response.layer_id)
                .with_clip_rect(response.interact_rect),
            egui::pos2(response.rect.left() + 14.0, response.rect.center().y),
            glyph,
            if response.hovered() { TEXT } else { SECONDARY },
        );
    }

    fn scroll_bars(&self, tab: &Panel) -> [bool; 2] {
        [
            false,
            !matches!(tab, Panel::Viewport | Panel::Graph | Panel::Console),
        ]
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Panel) {
        if matches!(tab, Panel::Viewport) {
            self.viewport(ui);
            return;
        }
        egui::Frame::new()
            .inner_margin(theme::PADDING)
            .show(ui, |ui| self.panel(ui, tab));
    }
}

impl Viewer<'_> {
    fn panel(&mut self, ui: &mut Ui, tab: &Panel) {
        match tab {
            Panel::Hierarchy => {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let width = (ui.available_width() - 96.0).max(24.0);
                    ui.add(
                        egui::TextEdit::singleline(self.search)
                            .desired_width(width)
                            .min_size(vec2(0.0, theme::ROW))
                            .hint_text(RichText::new("Search…").color(theme::MUTED)),
                    );
                    if icons::button(ui, Icon::Sort, self.hierarchy.sort, "Ordenar por nome")
                        .clicked()
                    {
                        self.hierarchy.sort = !self.hierarchy.sort;
                    }
                    if icons::button(
                        ui,
                        Icon::Filter,
                        self.hierarchy.players_only,
                        "Somente jogadores",
                    )
                    .clicked()
                    {
                        self.hierarchy.players_only = !self.hierarchy.players_only;
                    }
                    let more = icons::button(ui, Icon::More, false, "Ações da hierarquia");
                    egui::Popup::menu(&more).show(|ui| {
                        let editable = self.session.mode == Mode::Edit;
                        if ui
                            .add_enabled(
                                editable && self.root_present,
                                egui::Button::new("+ Entidade"),
                            )
                            .clicked()
                        {
                            *self.config_selected = false;
                            self.assets.clear_selection();
                            self.entity_action = Some(EntityAction::Create);
                            ui.close();
                        }
                        if let Some(entity) = *self.selected
                            && ui
                                .add_enabled(
                                    editable && !self.hierarchy.locked.contains(&entity),
                                    egui::Button::new("Remover"),
                                )
                                .clicked()
                        {
                            self.entity_action = Some(EntityAction::Delete(entity));
                            ui.close();
                        }
                    });
                });
                ui.add_space(4.0);
                let search = self.search.trim().to_lowercase();
                self.hierarchy_rows(ui, self.rows, &search, 0);
                if self.rows.is_empty() {
                    ui.label(theme::label("Aguardando a cena…"));
                }
            }
            Panel::Viewport => unreachable!(),
            Panel::Graph => self.flow_graph.draw(ui, self.flow_model),
            Panel::Inspector => {
                if *self.config_selected {
                    self.config.draw(ui);
                } else if let Some(row) = self
                    .selected
                    .and_then(|id| self.rows.iter().find_map(|row| row.find(id)))
                {
                    ui.heading(&row.label);
                    ui.label(
                        RichText::new(format!("Entity {}", row.entity))
                            .monospace()
                            .color(SECONDARY),
                    );
                    ui.separator();
                    self.inspector.draw(
                        ui,
                        row.entity,
                        self.session.mode == Mode::Edit
                            && !self.hierarchy.locked.contains(&row.entity),
                        &mut self.edits,
                    );
                } else if let Some(path) = self.assets.selected() {
                    ui.heading(path.file_name().unwrap_or_default().to_string_lossy());
                    ui.label(theme::label("CAMINHO DO ASSET").small());
                    ui.monospace(path.display().to_string());
                } else {
                    ui.label(theme::label("Selecione uma entidade."));
                }
            }
            Panel::Assets => {
                if let Some(path) = self.assets.draw(ui, self.asset_index) {
                    *self.config_selected =
                        path == std::path::Path::new("assets").join(game::PlayerConfig::PATH);
                    *self.selected = None;
                }
            }
            Panel::Console => {
                self.console.draw(ui, self.logs);
            }
        }
    }
    fn hierarchy_rows(&mut self, ui: &mut Ui, rows: &[Row], search: &str, depth: usize) {
        let mut rows: Vec<_> = rows.iter().collect();
        if self.hierarchy.sort {
            rows.sort_by_cached_key(|row| row.label.to_lowercase());
        }
        let filtering = !search.is_empty() || self.hierarchy.players_only;
        for row in rows {
            let has_children = !row.children.is_empty();
            let collapsed = self.hierarchy.collapsed.contains(&row.entity);
            if (!self.hierarchy.players_only || row.player)
                && row.label.to_lowercase().contains(search)
            {
                ui.push_id(row.entity, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.add_space((14.0 * depth as f32).min(ui.available_width() * 0.2));
                        if has_children && !filtering {
                            if ui
                                .add(
                                    egui::Button::new(if collapsed { "›" } else { "⌄" })
                                        .frame(false)
                                        .min_size(vec2(14.0, theme::ROW)),
                                )
                                .clicked()
                            {
                                if collapsed {
                                    self.hierarchy.collapsed.remove(&row.entity);
                                } else {
                                    self.hierarchy.collapsed.insert(row.entity);
                                }
                            }
                        } else {
                            ui.add_space(14.0);
                        }
                        let actions = if row.player { 50.0 } else { 26.0 };
                        let active = *self.selected == Some(row.entity) && !*self.config_selected;
                        if ui
                            .allocate_ui(
                                vec2((ui.available_width() - actions).max(12.0), theme::ROW),
                                |ui| {
                                    theme::row(
                                        ui,
                                        active,
                                        &row.label,
                                        if row.player {
                                            Icon::Player
                                        } else {
                                            Icon::Entity
                                        },
                                    )
                                },
                            )
                            .inner
                            .on_hover_text(&row.label)
                            .clicked()
                        {
                            *self.selected = Some(row.entity);
                            *self.config_selected = false;
                            self.assets.clear_selection();
                        }
                        if row.player {
                            let hidden = self.hierarchy.hidden.contains(&row.entity);
                            if icon_action(
                                ui,
                                if hidden {
                                    RowIcon::EyeOff
                                } else {
                                    RowIcon::Eye
                                },
                                hidden,
                                "Visibilidade da prévia",
                            )
                            .clicked()
                            {
                                if hidden {
                                    self.hierarchy.hidden.remove(&row.entity);
                                } else {
                                    self.hierarchy.hidden.insert(row.entity);
                                }
                            }
                        }
                        let locked = self.hierarchy.locked.contains(&row.entity);
                        if icon_action(ui, RowIcon::Lock, locked, "Bloquear edição").clicked() {
                            if locked {
                                self.hierarchy.locked.remove(&row.entity);
                            } else {
                                self.hierarchy.locked.insert(row.entity);
                            }
                        }
                    });
                });
            }
            if filtering || !collapsed {
                self.hierarchy_rows(
                    ui,
                    &row.children,
                    search,
                    if filtering { 0 } else { depth + 1 },
                );
            }
        }
    }

    fn viewport(&mut self, ui: &mut Ui) {
        let target = self.target.filter(|target| {
            self.session.mode == Mode::Edit
                && *self.selected == Some(target.entity)
                && !*self.config_selected
                && !self.hierarchy.locked.contains(&target.entity)
                && !self.hierarchy.hidden.contains(&target.entity)
        });
        let size = ui.available_size();
        if size.x < 2.0 || size.y < 2.0 {
            return;
        }
        let rounding = egui::CornerRadius {
            sw: theme::PANEL_RADIUS,
            se: theme::PANEL_RADIUS,
            ..egui::CornerRadius::ZERO
        };
        let response = if let Some(texture) = self.texture {
            self.view_size = Some(size);
            ui.add(
                egui::Image::new(egui::load::SizedTexture::new(texture, size))
                    .fit_to_exact_size(size)
                    .corner_radius(rounding)
                    .sense(egui::Sense::click_and_drag()),
            )
        } else {
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
            ui.painter().rect_filled(rect, rounding, theme::DEEP);
            response
        };
        self.fly
            .navigate(&response, ui.input(|input| input.stable_dt));
        self.transformed = self
            .gizmo
            .draw(&response, self.camera, self.fly.transform, target);

        let rect = response.rect;
        let mut overlay = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("viewport_overlay")
                .max_rect(egui::Rect::from_min_max(
                    rect.min + vec2(10.0, 10.0),
                    rect.max - vec2(10.0, 8.0),
                ))
                .layout(egui::Layout::top_down(egui::Align::Center)),
        );
        overlay.set_clip_rect(overlay.clip_rect().intersect(rect));
        overlay.horizontal_wrapped(|ui| {
            pill(ui, |ui| {
                let options = icons::button(ui, Icon::View, false, "Opções da Viewport");
                egui::Popup::menu(&options).show(|ui| {
                    ui.checkbox(self.grid_visible, "Grid");
                    if ui.button("Reset câmera").clicked() {
                        *self.fly = FlyCamera::default();
                        ui.close();
                    }
                    ui.label(theme::label("RMB · WASD / QE"));
                    ui.label(theme::label("Scroll · Zoom"));
                });
            });
            if size.x > 490.0 {
                ui.add_space((size.x * 0.5 - 211.0).max(0.0));
            }
            pill(ui, |ui| {
                self.gizmo.toolbar(ui, target.is_some());
            });
            pill(ui, |ui| {
                ui.scope(|ui| {
                    let widgets = &mut ui.visuals_mut().widgets;
                    for (w, color) in [
                        (&mut widgets.inactive, RUST),
                        (&mut widgets.hovered, theme::RUST_HOVER),
                        (&mut widgets.active, theme::RUST_HOVER),
                    ] {
                        w.weak_bg_fill = color;
                        w.bg_fill = color;
                        w.bg_stroke = egui::Stroke::NONE;
                    }
                    if ui
                        .add_enabled(
                            !self.rows.is_empty() && self.session.mode != Mode::Play,
                            egui::Button::new(RichText::new("▶ Play").strong().color(CANVAS))
                                .frame(true)
                                .corner_radius(theme::PILL_RADIUS),
                        )
                        .clicked()
                    {
                        self.action = Some(Action::Play);
                    }
                });
                if ui
                    .add_enabled_ui(self.session.mode == Mode::Play, |ui| {
                        icons::button(ui, Icon::Pause, false, "Pause").clicked()
                    })
                    .inner
                {
                    self.action = Some(Action::Pause);
                }
                if ui
                    .add_enabled_ui(self.session.mode != Mode::Edit, |ui| {
                        icons::button(ui, Icon::Stop, false, "Stop").clicked()
                    })
                    .inner
                {
                    self.action = Some(Action::Stop);
                }
            });
        });
        if size.x >= 570.0 && size.y >= 90.0 {
            gizmo::orientation(
                ui,
                rect.right_top() + vec2(-38.0, 38.0),
                self.fly.transform.rotation,
            );
        }
        if size.y >= 78.0 {
            ui.painter_at(rect).text(
                rect.left_bottom() + vec2(14.0, -12.0),
                egui::Align2::LEFT_BOTTOM,
                format!("Perspective  /  {:?}", self.session.mode),
                egui::FontId::monospace(11.0),
                TEXT,
            );
        }
    }
}

fn pill(ui: &mut Ui, content: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(SURFACE.gamma_multiply(0.96))
        .corner_radius(theme::PILL_RADIUS)
        .inner_margin(egui::Margin::symmetric(4, 3))
        .shadow(egui::Shadow {
            offset: [0, 2],
            blur: 8,
            spread: 0,
            color: Color32::from_black_alpha(66),
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                content(ui);
            });
        });
}

enum RowIcon {
    Eye,
    EyeOff,
    Lock,
}

fn icon_action(ui: &mut Ui, icon: RowIcon, active: bool, tooltip: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(22.0, theme::ROW), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, true, active, tooltip)
    });
    let c = rect.center();
    let color = if response.hovered() || active {
        TEXT
    } else {
        theme::MUTED
    };
    let stroke = egui::Stroke::new(1.5, color);
    let p = ui.painter();
    match icon {
        RowIcon::Eye | RowIcon::EyeOff => {
            p.add(egui::Shape::closed_line(
                [
                    (-6.0, 0.0),
                    (-3.0, -3.5),
                    (0.0, -4.5),
                    (3.0, -3.5),
                    (6.0, 0.0),
                    (3.0, 3.5),
                    (0.0, 4.5),
                    (-3.0, 3.5),
                ]
                .map(|(x, y)| c + vec2(x, y))
                .to_vec(),
                stroke,
            ));
            p.circle_filled(c, 1.65, color);
            if matches!(icon, RowIcon::EyeOff) {
                p.line_segment([c + vec2(-6.0, 5.0), c + vec2(6.0, -5.0)], stroke);
            }
        }
        RowIcon::Lock => {
            p.rect_stroke(
                egui::Rect::from_center_size(c + vec2(0.0, 2.0), vec2(9.0, 7.0)),
                2,
                stroke,
                egui::StrokeKind::Inside,
            );
            let x = if active { 0.0 } else { -2.5 };
            p.add(egui::Shape::line(
                [
                    (x - 2.5, -1.5),
                    (x - 2.5, -4.0),
                    (x - 1.0, -5.5),
                    (x + 1.0, -5.5),
                    (x + 2.5, -4.0),
                    (x + 2.5, -1.5),
                ]
                .map(|(x, y)| c + vec2(x, y))
                .to_vec(),
                stroke,
            ));
        }
    }
    response.on_hover_text(tooltip)
}
