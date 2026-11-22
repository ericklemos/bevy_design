use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    world_serialization::{DynamicWorldRoot, WorldInstance, WorldInstanceSpawner},
};
use bevy_egui::{EguiContexts, egui};
use egui::{Color32, RichText, Ui, vec2};
use egui_dock::{DockArea, DockState, NodeIndex, TabViewer};
use game::Health;
use std::collections::HashSet;

use crate::{
    EditorOnly,
    config::ConfigEditor,
    flow_graph::{FlowGraph, Model, View as GraphView},
    gizmo::{self, Gizmo},
    icons::{self, Icon},
    inspector::{Edit, Inspector},
    project_picker::Picker,
    session::{self, Action, Mode, Session},
    theme::{self, CANVAS, RUST, SECONDARY, SURFACE, TEXT},
    viewport::{EditCamera, FlyCamera, Viewport},
};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum Panel {
    Hierarchy,
    Viewport,
    Graph,
    Inspector,
    Assets,
    Console,
}

impl Panel {
    fn title(self) -> &'static str {
        match self {
            Self::Hierarchy => "Hierarchy",
            Self::Viewport => "Viewport",
            Self::Graph => "Flow Graph",
            Self::Inspector => "Inspector",
            Self::Assets => "Asset Browser",
            Self::Console => "Console",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Self::Hierarchy => Icon::Tree,
            Self::Viewport => Icon::View,
            Self::Graph => Icon::Graph,
            Self::Inspector => Icon::Inspector,
            Self::Assets => Icon::Folder,
            Self::Console => Icon::Terminal,
        }
    }
}

/// Top-level workspace screens, in the spirit of Blender's workspaces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Screen {
    #[default]
    Building,
    Scripting,
}

impl Screen {
    fn label(self) -> &'static str {
        match self {
            Self::Building => "Building",
            Self::Scripting => "Scripting",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Building => "Construir a cena: hierarquia, viewport e inspetor.",
            Self::Scripting => "Lógica e Flow Graph: sistemas, queries e observers.",
        }
    }
}

fn default_building_dock() -> DockState<Panel> {
    const LEFT: f32 = 0.20;
    const CENTER: f32 = 0.58;
    const TOP: f32 = 0.58;
    let mut dock = DockState::new(vec![Panel::Viewport]);
    let tree = dock.main_surface_mut();
    // The fraction is the left child's share, even when the new tab is on that side.
    let [center, left] = tree.split_left(NodeIndex::root(), LEFT, vec![Panel::Hierarchy]);
    let [center, _] = tree.split_right(center, CENTER / (1.0 - LEFT), vec![Panel::Inspector]);
    // The Flow Graph belongs to the Scripting screen, so Building starts without it.
    tree.split_below(center, TOP, vec![Panel::Console]);
    tree.split_below(left, TOP, vec![Panel::Assets]);
    dock
}

fn default_scripting_dock() -> DockState<Panel> {
    const GRAPH: f32 = 0.78;
    const MINIMIZED_CONSOLE: f32 = 0.88;
    let mut dock = DockState::new(vec![Panel::Graph]);
    let tree = dock.main_surface_mut();
    // The graph replaces the viewport; the console is a collapsed strip at the bottom.
    tree.split_right(NodeIndex::root(), GRAPH, vec![Panel::Inspector]);
    tree.split_below(NodeIndex::root(), MINIMIZED_CONSOLE, vec![Panel::Console]);
    dock
}

fn header(
    ui: &mut Ui,
    workspace: &mut Workspace,
    session: &Session,
    projects: &mut Picker,
    commands: &mut Commands,
) {
    let rect = ui.max_rect();
    let height = rect.height();
    let third = rect.width() / 3.0;
    let center_x = rect.left() + third;
    let end_x = rect.left() + third * 2.0;
    let mut screen = workspace.screen;

    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(egui::Rect::from_min_size(rect.min, vec2(third, height)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Fechar editor").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        commands.write_message(AppExit::Success);
                    }
                });
                if ui.button("Procurar").clicked() {
                    projects.choose(false);
                }
                ui.menu_button("Editors", |ui| {
                    if ui.button("Restaurar layout inicial").clicked() {
                        workspace.reset_layout();
                        ui.close();
                    }
                });
                ui.menu_button("Help", |ui| {
                    ui.label("Viewport: arraste com o botão direito.");
                    ui.label("WASD move; Q/E desce/sobe; roda aproxima.");
                });
            });
        },
    );

    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(egui::Rect::from_min_size(
                egui::pos2(center_x, rect.top()),
                vec2(third, height),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| screen_section(ui, third, &mut screen),
    );

    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(egui::Rect::from_min_size(
                egui::pos2(end_x, rect.top()),
                vec2(rect.right() - end_x, height),
            ))
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
        |ui| session_status(ui, session),
    );

    if screen != workspace.screen {
        workspace.set_screen(screen);
    }
}

const SCREEN_TAB_WIDTH: f32 = 96.0;
const SCREEN_SWITCH_WIDTH: f32 = SCREEN_TAB_WIDTH * 2.0 + 1.0 + 6.0;

fn screen_section(ui: &mut Ui, width: f32, screen: &mut Screen) {
    ui.add_space(((width - SCREEN_SWITCH_WIDTH) * 0.5).max(0.0));
    screen_switch(ui, screen);
}

fn screen_switch(ui: &mut Ui, screen: &mut Screen) {
    egui::Frame::new()
        .fill(theme::DEEP)
        .corner_radius(theme::PILL_RADIUS)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .inner_margin(egui::Margin::symmetric(3, 2))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            ui.horizontal(|ui| {
                for option in [Screen::Building, Screen::Scripting] {
                    if screen_tab(ui, option, *screen == option).clicked() {
                        *screen = option;
                    }
                }
            });
        });
}

fn screen_tab(ui: &mut Ui, screen: Screen, selected: bool) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(SCREEN_TAB_WIDTH, 22.0), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            selected,
            screen.label(),
        )
    });
    let radius = theme::PILL_RADIUS;
    let fill = if selected {
        SURFACE
    } else if response.hovered() {
        theme::HOVER
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, radius, fill);
    if selected {
        ui.painter().rect_stroke(
            rect,
            radius,
            egui::Stroke::new(1.0, theme::RUST_SOFT),
            egui::StrokeKind::Inside,
        );
    }
    let color = if selected || response.hovered() {
        TEXT
    } else {
        SECONDARY
    };
    ui.painter().circle_filled(
        egui::pos2(rect.left() + 14.0, rect.center().y),
        2.5,
        if selected { RUST } else { theme::MUTED },
    );
    let galley = ui.painter().layout_no_wrap(
        screen.label().to_owned(),
        egui::FontId::proportional(12.0),
        color,
    );
    ui.painter().galley(
        egui::pos2(rect.left() + 24.0, rect.center().y - galley.size().y * 0.5),
        galley,
        color,
    );
    response.on_hover_text(screen.hint())
}

fn session_status(ui: &mut Ui, session: &Session) {
    let (label, color) = match session.mode {
        Mode::Edit => ("Edição", theme::SECONDARY),
        Mode::Play => ("Simulação", RUST),
        Mode::Pause => ("Pausada", theme::WARNING),
    };
    egui::Frame::new()
        .fill(theme::ISLAND)
        .corner_radius(theme::PILL_RADIUS)
        .inner_margin(egui::Margin::symmetric(10, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(6.0, 6.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 3.0, color);
                ui.label(egui::RichText::new(label).size(11.0).color(TEXT));
            });
        });
}

fn tool_is_active(dock: &DockState<Panel>, panel: Panel) -> bool {
    dock.find_tab(&panel).is_some_and(|path| {
        dock.leaf(path.node_path())
            .is_ok_and(|leaf| leaf.active == path.tab)
    })
}

fn toggle_tool(dock: &mut DockState<Panel>, panel: Panel) {
    if let Some(path) = dock.find_tab(&panel) {
        if tool_is_active(dock, panel) {
            dock.remove_tab(path);
        } else {
            let _ = dock.set_active_tab(path);
            dock.set_focused_node_and_surface(path.node_path());
        }
        return;
    }

    // Prefer the companion pane when it is still docked; don't reset the rest of the layout.
    let companion = match panel {
        Panel::Hierarchy => Some(Panel::Assets),
        Panel::Assets => Some(Panel::Hierarchy),
        Panel::Graph => Some(Panel::Console),
        Panel::Console => Some(Panel::Graph),
        Panel::Inspector => None,
        Panel::Viewport => unreachable!("the viewport isn't on the toolbars"),
    }
    .and_then(|panel| dock.find_main_surface_tab(&panel).map(|(node, _)| node));
    let viewport = dock
        .find_main_surface_tab(&Panel::Viewport)
        .map(|(node, _)| node);
    let tree = dock.main_surface_mut();
    let root = NodeIndex::root();
    if tree.is_empty() {
        tree.push_to_first_leaf(panel);
        return;
    }
    let tabs = vec![panel];
    match (panel, companion) {
        (Panel::Hierarchy, Some(node)) => tree.split_above(node, 0.58, tabs),
        (Panel::Assets, Some(node)) => tree.split_below(node, 0.58, tabs),
        (Panel::Graph, Some(node)) => tree.split_left(node, 0.57, tabs),
        (Panel::Console, Some(node)) => tree.split_right(node, 0.57, tabs),
        (Panel::Hierarchy | Panel::Assets, None) => tree.split_left(root, 0.20, tabs),
        (Panel::Inspector, _) => tree.split_right(root, 0.78, tabs),
        (Panel::Graph | Panel::Console, None) => {
            tree.split_below(viewport.unwrap_or(root), 0.58, tabs)
        }
        (Panel::Viewport, _) => unreachable!(),
    };
}

fn tool_rail(
    outer: &mut Ui,
    edge: egui::Panel,
    dock: &mut DockState<Panel>,
    upper: &[Panel],
    lower: &[Panel],
) {
    edge.resizable(false)
        .exact_size(theme::TOOL_RAIL)
        .frame(
            egui::Frame::new()
                .fill(theme::CANVAS)
                .inner_margin(egui::Margin::symmetric(4, 8)),
        )
        .show(outer, |ui| {
            let rect = ui.max_rect();
            let top =
                egui::Rect::from_min_size(rect.min, vec2(rect.width(), rail_group_height(upper)));
            let height = rail_group_height(lower);
            let bottom = egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.bottom() - height),
                vec2(rect.width(), height),
            );
            for (rect, panels) in [(top, upper), (bottom, lower)] {
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(rect)
                        .layout(egui::Layout::top_down(egui::Align::Center)),
                    |ui| rail_group(ui, dock, panels),
                );
            }
        });
}

fn view_rail(outer: &mut Ui, edge: egui::Panel, flow: &mut FlowGraph) {
    edge.resizable(false)
        .exact_size(theme::TOOL_RAIL)
        .frame(
            egui::Frame::new()
                .fill(theme::CANVAS)
                .inner_margin(egui::Margin::symmetric(4, 8)),
        )
        .show(outer, |ui| {
            let rect = ui.max_rect();
            let height = rail_group_count_height(GraphView::ALL.len());
            let top = egui::Rect::from_min_size(rect.min, vec2(rect.width(), height));
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(top)
                    .layout(egui::Layout::top_down(egui::Align::Center)),
                |ui| {
                    theme::island(egui::Margin::symmetric(4, 4)).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.vertical_centered(|ui| {
                            for view in GraphView::ALL {
                                let label = format!("{}\n{}", view.label(), view.hint());
                                let active = flow.view() == view;
                                if icons::sized_button(
                                    ui,
                                    vec2(32.0, 32.0),
                                    view.icon(),
                                    active,
                                    &label,
                                )
                                .clicked()
                                {
                                    flow.set_view(view);
                                }
                            }
                        });
                    });
                },
            );
        });
}

fn rail_group_height(panels: &[Panel]) -> f32 {
    rail_group_count_height(panels.len())
}

fn rail_group_count_height(count: usize) -> f32 {
    let count = count as f32;
    (count * 32.0 + (count - 1.0).max(0.0) * 2.0 + 8.0).max(0.0)
}

fn rail_group(ui: &mut Ui, dock: &mut DockState<Panel>, panels: &[Panel]) {
    theme::island(egui::Margin::symmetric(4, 4)).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.vertical_centered(|ui| {
            for &panel in panels {
                let active = tool_is_active(dock, panel);
                if icons::sized_button(ui, vec2(32.0, 32.0), panel.icon(), active, panel.title())
                    .clicked()
                {
                    toggle_tool(dock, panel);
                }
            }
        });
    });
}

#[cfg(test)]
mod dock_tests {
    use super::*;
    use egui_dock::Node;

    #[test]
    fn initial_dock_gives_hierarchy_the_narrow_left_column() {
        let dock = default_building_dock();
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
    fn building_layout_starts_without_the_flow_graph() {
        let dock = default_building_dock();
        let tree = dock.main_surface();
        let center = NodeIndex::root().right().left();
        assert!(matches!(
            tree[center.left()].tabs(),
            Some([Panel::Viewport])
        ));
        assert!(matches!(
            tree[center.right()].tabs(),
            Some([Panel::Console])
        ));
        assert!(dock.find_tab(&Panel::Graph).is_none());
    }

    #[test]
    fn scripting_screen_replaces_viewport_and_hides_building_tools() {
        let mut workspace = Workspace::default();
        assert_eq!(workspace.screen, Screen::Building);
        assert!(workspace.dock().find_tab(&Panel::Graph).is_none());
        assert!(workspace.dock().find_tab(&Panel::Viewport).is_some());

        workspace.set_screen(Screen::Scripting);
        let dock = workspace.dock();
        assert!(dock.find_tab(&Panel::Graph).is_some());
        assert!(tool_is_active(dock, Panel::Graph));
        assert!(dock.find_tab(&Panel::Viewport).is_none());
        assert!(dock.find_tab(&Panel::Hierarchy).is_none());
        assert!(dock.find_tab(&Panel::Assets).is_none());
        assert!(dock.find_tab(&Panel::Inspector).is_some());
        assert!(dock.find_tab(&Panel::Console).is_some());

        workspace.set_screen(Screen::Building);
        assert!(workspace.dock().find_tab(&Panel::Graph).is_none());
        assert!(workspace.dock().find_tab(&Panel::Viewport).is_some());
        assert!(workspace.dock().find_tab(&Panel::Console).is_some());
    }

    #[test]
    fn restoring_the_layout_restores_both_screens() {
        let mut workspace = Workspace::default();
        workspace.set_screen(Screen::Scripting);
        workspace.reset_layout();
        assert!(workspace.dock().find_tab(&Panel::Graph).is_some());
        assert!(workspace.dock().find_tab(&Panel::Viewport).is_none());
        workspace.set_screen(Screen::Building);
        assert!(workspace.dock().find_tab(&Panel::Viewport).is_some());
        assert!(workspace.dock().find_tab(&Panel::Graph).is_none());
    }

    #[test]
    fn header_only_switches_modes_from_the_center_section() {
        fn pass(
            ctx: &egui::Context,
            screen: egui::Rect,
            workspace: &mut Workspace,
            session: &Session,
            projects: &mut Picker,
            commands: &mut Commands,
            events: Vec<egui::Event>,
        ) {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ui| header(ui, workspace, session, projects, commands),
            );
            output.textures_delta.clear();
        }

        fn send_click(
            ctx: &egui::Context,
            screen: egui::Rect,
            workspace: &mut Workspace,
            session: &Session,
            projects: &mut Picker,
            commands: &mut Commands,
            x: f32,
        ) {
            let pos = egui::pos2(x, 16.0);
            pass(
                ctx,
                screen,
                workspace,
                session,
                projects,
                commands,
                vec![egui::Event::PointerMoved(pos)],
            );
            for pressed in [true, false] {
                pass(
                    ctx,
                    screen,
                    workspace,
                    session,
                    projects,
                    commands,
                    vec![egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    }],
                );
            }
        }

        let ctx = egui::Context::default();
        icons::add_font(&ctx);
        theme::install(&ctx);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(900.0, 32.0));
        let mut world = World::new();
        let mut commands = world.commands();
        let mut workspace = Workspace::default();
        let session = Session::default();
        let mut projects = Picker::default();
        let mut click = |workspace: &mut Workspace, x| {
            send_click(
                &ctx,
                screen,
                workspace,
                &session,
                &mut projects,
                &mut commands,
                x,
            );
        };

        click(&mut workspace, 250.0);
        assert_eq!(workspace.screen, Screen::Building);
        click(&mut workspace, 750.0);
        assert_eq!(workspace.screen, Screen::Building);

        click(&mut workspace, 498.0);
        assert_eq!(workspace.screen, Screen::Scripting);
        assert!(workspace.dock().find_tab(&Panel::Graph).is_some());
        assert!(workspace.dock().find_tab(&Panel::Viewport).is_none());

        click(&mut workspace, 401.0);
        assert_eq!(workspace.screen, Screen::Building);
        assert!(workspace.dock().find_tab(&Panel::Graph).is_none());
        assert!(workspace.dock().find_tab(&Panel::Viewport).is_some());
    }

    #[test]
    fn sidebar_reopens_tools_without_replacing_the_remaining_layout() {
        let mut dock = default_building_dock();
        toggle_tool(&mut dock, Panel::Hierarchy);
        assert!(dock.find_tab(&Panel::Hierarchy).is_none());
        assert!(tool_is_active(&dock, Panel::Assets));

        toggle_tool(&mut dock, Panel::Hierarchy);
        let (hierarchy, _) = dock.find_main_surface_tab(&Panel::Hierarchy).unwrap();
        let (assets, _) = dock.find_main_surface_tab(&Panel::Assets).unwrap();
        assert_eq!(hierarchy, assets.parent().unwrap().left());
        assert!(tool_is_active(&dock, Panel::Viewport));
    }

    #[test]
    fn sidebar_activates_an_inactive_tab_before_hiding_it() {
        let mut dock = DockState::new(vec![Panel::Graph, Panel::Console]);
        assert!(!tool_is_active(&dock, Panel::Console));

        toggle_tool(&mut dock, Panel::Console);
        assert!(tool_is_active(&dock, Panel::Console));
        assert!(dock.find_tab(&Panel::Graph).is_some());

        toggle_tool(&mut dock, Panel::Console);
        assert!(dock.find_tab(&Panel::Console).is_none());
        assert!(tool_is_active(&dock, Panel::Graph));
    }

    #[test]
    fn scripting_view_rail_switches_the_graph_view() {
        let ctx = egui::Context::default();
        icons::add_font(&ctx);
        theme::install(&ctx);
        let temp = tempfile::tempdir().unwrap();
        let mut flow = FlowGraph::new(temp.path().join("layout.ron"), None);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(640.0, 480.0));
        let pass = |flow: &mut FlowGraph, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ui| view_rail(ui, egui::Panel::left("test_views"), flow),
            );
            output.textures_delta.clear();
        };
        let click = |flow: &mut FlowGraph, y: f32| {
            let pos = egui::pos2(24.0, y);
            pass(flow, vec![egui::Event::PointerMoved(pos)]);
            for pressed in [true, false] {
                pass(
                    flow,
                    vec![egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    }],
                );
            }
        };
        click(&mut flow, 62.0);
        assert_eq!(flow.view(), GraphView::Access);
        click(&mut flow, 96.0);
        assert_eq!(flow.view(), GraphView::Observers);
        click(&mut flow, 28.0);
        assert_eq!(flow.view(), GraphView::Order);
    }

    #[test]
    fn rail_buttons_are_usable_at_both_ends() {
        let ctx = egui::Context::default();
        icons::add_font(&ctx);
        theme::install(&ctx);
        let mut dock = default_building_dock();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(640.0, 480.0));
        let pass = |dock: &mut DockState<Panel>, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ui| {
                    tool_rail(
                        ui,
                        egui::Panel::right("test_tools"),
                        dock,
                        &[Panel::Inspector],
                        &[Panel::Graph, Panel::Console],
                    );
                },
            );
            output.textures_delta.clear();
        };
        for y in [22.0, 458.0, 418.0] {
            let pos = egui::pos2(620.0, y);
            pass(&mut dock, vec![egui::Event::PointerMoved(pos)]);
            for pressed in [true, false] {
                pass(
                    &mut dock,
                    vec![egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    }],
                );
            }
        }
        assert!(dock.find_tab(&Panel::Inspector).is_none());
        assert!(dock.find_tab(&Panel::Console).is_none());
        assert!(dock.find_tab(&Panel::Graph).is_some());
    }
}

#[derive(Resource)]
pub(crate) struct Workspace {
    building: DockState<Panel>,
    scripting: DockState<Panel>,
    screen: Screen,
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
            building: default_building_dock(),
            scripting: default_scripting_dock(),
            screen: Screen::default(),
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

    #[cfg(test)]
    fn dock(&self) -> &DockState<Panel> {
        match self.screen {
            Screen::Building => &self.building,
            Screen::Scripting => &self.scripting,
        }
    }

    fn dock_mut(&mut self) -> &mut DockState<Panel> {
        match self.screen {
            Screen::Building => &mut self.building,
            Screen::Scripting => &mut self.scripting,
        }
    }

    fn set_screen(&mut self, screen: Screen) {
        self.screen = screen;
    }

    fn reset_layout(&mut self) {
        self.building = default_building_dock();
        self.scripting = default_scripting_dock();
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
    config: Option<ResMut<'w, ConfigEditor>>,
    scene_spawner: Res<'w, WorldInstanceSpawner>,
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
    roots: Query<(Entity, &WorldInstance), With<DynamicWorldRoot>>,
    children: Query<&Children>,
    nodes: SceneNodes,
    mut viewport: ResMut<Viewport>,
    mut fly: ResMut<FlyCamera>,
    mut camera: Single<(&Camera, &mut Transform), (With<EditCamera>, With<EditorOnly>)>,
    mut tools: Tools,
    mut commands: Commands,
    mut themed: Local<bool>,
    mut projects: Local<Picker>,
) -> Result {
    let root = session.restored_root.or_else(|| {
        let (entity, instance) = roots.single().ok()?;
        tools
            .scene_spawner
            .instance_is_ready(**instance)
            .then_some(entity)
    });
    let rows: Vec<_> = root
        .and_then(|root| children.get(root).ok())
        .into_iter()
        .flat_map(Children::iter)
        .filter_map(|child| Row::read(child, &nodes))
        .collect();
    let texture = contexts.image_id(&viewport.image);
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
            header(ui, &mut workspace, &session, &mut projects, &mut commands);
        });
    egui::Panel::bottom("status")
        .resizable(false)
        .exact_size(theme::STATUS_BAR)
        .frame(
            egui::Frame::new()
                .fill(theme::CANVAS)
                .inner_margin(egui::Margin::symmetric(theme::GUTTER, 4)),
        )
        .show(&mut outer, |ui| {
            egui::Frame::new()
                .fill(theme::ISLAND)
                .corner_radius(theme::PILL_RADIUS)
                .inner_margin(egui::Margin::symmetric(10, 0))
                .show(ui, |ui| {
                    ui.set_min_size(vec2(ui.available_width(), 16.0));
                });
        });
    let screen = workspace.screen;
    match screen {
        Screen::Building => {
            tool_rail(
                &mut outer,
                egui::Panel::left("building_left_tools"),
                workspace.dock_mut(),
                &[Panel::Hierarchy],
                &[Panel::Assets],
            );
            tool_rail(
                &mut outer,
                egui::Panel::right("building_right_tools"),
                workspace.dock_mut(),
                &[Panel::Inspector],
                &[Panel::Console],
            );
        }
        Screen::Scripting => {
            view_rail(
                &mut outer,
                egui::Panel::left("scripting_views"),
                &mut tools.flow_graph,
            );
            tool_rail(
                &mut outer,
                egui::Panel::right("scripting_right_tools"),
                workspace.dock_mut(),
                &[Panel::Inspector],
                &[Panel::Graph, Panel::Console],
            );
        }
    }

    let Workspace {
        building,
        scripting,
        selected,
        search,
        config_selected,
        gizmo,
        hierarchy,
        assets,
        console,
        initialized_selection,
        grid_visible,
        ..
    } = &mut *workspace;
    let dock = match screen {
        Screen::Building => building,
        Screen::Scripting => scripting,
    };
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
        screen,
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
        config: tools.config.as_deref_mut(),
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
                .show_leaf_collapse_buttons(false)
                .show_leaf_close_all_buttons(false)
                .show_close_buttons(false)
                .show_inside(ui, &mut viewer);
        });
    viewport.request_size(viewer.view_size, ctx.pixels_per_point());
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
    projects.window(ctx);
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
    screen: Screen,
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
    config: Option<&'a mut ConfigEditor>,
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
            tab.title(),
            18.0,
            egui::TextFormat::simple(egui::FontId::proportional(12.0), TEXT),
        );
        job.into()
    }

    fn on_tab_button(&mut self, tab: &mut Panel, response: &egui::Response) {
        icons::paint(
            &response
                .ctx
                .layer_painter(response.layer_id)
                .with_clip_rect(response.interact_rect),
            egui::pos2(response.rect.left() + 14.0, response.rect.center().y),
            tab.icon(),
            if response.hovered() { TEXT } else { SECONDARY },
        );
    }

    fn is_closeable(&self, tab: &Panel) -> bool {
        *tab != Panel::Viewport
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
                    ui.label(theme::label(if self.root_present {
                        "Nenhuma entidade na cena."
                    } else {
                        "Aguardando a cena…"
                    }));
                }
            }
            Panel::Viewport => unreachable!(),
            Panel::Graph => self.flow_graph.draw(ui, self.flow_model),
            Panel::Inspector => {
                if self.screen == Screen::Scripting {
                    self.flow_graph.draw_inspector(ui, self.flow_model);
                } else if *self.config_selected
                    && let Some(config) = self.config.as_deref_mut()
                {
                    config.draw(ui);
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
                    ui.add_space(8.0);
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
                    *self.config_selected = self.config.is_some()
                        && path == std::path::Path::new("assets").join(game::PlayerConfig::PATH);
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
                            let (icon, label) = if collapsed {
                                (Icon::Collapsed, "Expandir entidade")
                            } else {
                                (Icon::Expanded, "Recolher entidade")
                            };
                            if icons::sized_button(ui, vec2(14.0, theme::ROW), icon, false, label)
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
                            if icons::sized_button(
                                ui,
                                vec2(22.0, theme::ROW),
                                if hidden { Icon::EyeOff } else { Icon::Eye },
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
                        let (icon, label) = if locked {
                            (Icon::Lock, "Desbloquear edição")
                        } else {
                            (Icon::LockOpen, "Bloquear edição")
                        };
                        if icons::sized_button(ui, vec2(22.0, theme::ROW), icon, locked, label)
                            .clicked()
                        {
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
        let rounding = egui::CornerRadius::same(theme::PANEL_RADIUS);
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
                            egui::Button::new((
                                Icon::Play.text().color(CANVAS),
                                RichText::new("Play").strong().color(CANVAS),
                            ))
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
