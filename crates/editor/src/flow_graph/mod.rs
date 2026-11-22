mod layout;
mod model;

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::PathBuf,
    time::{Duration, Instant},
};

use bevy::{
    app::{AppLabel, SubApp},
    prelude::*,
};
use bevy_egui::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2, pos2, vec2};

use layout::Layout;
use model::{Edge, EdgeKind, Graph, Node, NodeKind};
pub(crate) use model::{Model, Origins};

use crate::icons::{self, Icon};
use crate::theme::{self, BORDER, DEEP, HOVER, RUST, RUST_SOFT, SECONDARY, SURFACE, TEXT};

const NODE_SIZE: Vec2 = vec2(218.0, 58.0);
const MEMBERSHIP: Color32 = Color32::from_rgb(115, 110, 106);
const READ: Color32 = Color32::from_rgb(124, 165, 210);
const WRITE: Color32 = Color32::from_rgb(196, 149, 224);
const LISTEN: Color32 = Color32::from_rgb(99, 199, 177);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum View {
    #[default]
    Order,
    Access,
    Observers,
}

#[derive(Clone, Copy)]
struct Camera {
    zoom: f32,
    offset: Vec2,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            zoom: 0.85,
            offset: Vec2::ZERO,
        }
    }
}

#[derive(Resource)]
pub(crate) struct FlowGraph {
    view: View,
    schedule: String,
    plugin: Option<String>,
    set: Option<String>,
    show_common: bool,
    layout: Layout,
    camera: Camera,
    selected: Option<String>,
}

impl FlowGraph {
    pub(crate) fn new(path: PathBuf, plugin: Option<&str>) -> Self {
        Self {
            view: View::Order,
            schedule: "Update".into(),
            plugin: plugin.map(str::to_owned),
            set: None,
            show_common: false,
            layout: Layout::load(path),
            camera: Camera::default(),
            selected: None,
        }
    }

    pub(crate) fn draw(&mut self, ui: &mut egui::Ui, model: &Model) {
        ui.horizontal_wrapped(|ui| {
            let previous = self.view;
            ui.selectable_value(&mut self.view, View::Order, "Agendador");
            ui.selectable_value(&mut self.view, View::Access, "Queries");
            ui.selectable_value(&mut self.view, View::Observers, "Observers");
            if self.view != previous {
                self.camera = Camera::default();
            }
            let more = icons::button(ui, Icon::More, false, "Ações do grafo");
            egui::Popup::menu(&more).show(|ui| {
                if ui.button("Recentralizar").clicked() {
                    self.camera = Camera::default();
                    ui.close();
                }
                if ui.button("Resetar nós").clicked() {
                    self.layout.reset(&self.prefix());
                    self.camera = Camera::default();
                    ui.close();
                }
            });
        });
        self.filters(ui, model);

        let graph = self.visible(model);
        let help = match self.view {
            View::Order => "Laranja: ordem declarada   ·   Cinza: pertence ao conjunto",
            View::Access => "Azul: leitura   ·   Roxo: escrita   ·   Acesso não implica ordem",
            View::Observers => "Verde: assinatura evento → Observer   ·   Emissor não inferido",
        };
        ui.label(
            egui::RichText::new(format!(
                "{} nós / {} conexões",
                graph.nodes.len(),
                graph.edges.len()
            ))
            .small()
            .color(SECONDARY),
        )
        .on_hover_text(help);
        if let Some(error) = &self.layout.status {
            ui.label(error);
        }
        self.canvas(ui, &graph);
        if !ui.input(|input| input.pointer.any_down()) {
            self.layout.save_if_dirty();
        }
    }

    fn filters(&mut self, ui: &mut egui::Ui, model: &Model) {
        ui.horizontal_wrapped(|ui| {
            if self.view != View::Observers {
                let previous = self.schedule.clone();
                if !model.schedules.contains_key(&self.schedule)
                    && let Some(schedule) = model.schedules.keys().next()
                {
                    self.schedule.clone_from(schedule);
                }
                egui::ComboBox::from_id_salt("flow_schedule")
                    .selected_text(&self.schedule)
                    .show_ui(ui, |ui| {
                        for schedule in model.schedules.keys() {
                            ui.selectable_value(&mut self.schedule, schedule.clone(), schedule);
                        }
                    });
                if self.schedule != previous {
                    self.set = None;
                    self.camera = Camera::default();
                }
            }
            let origins: BTreeSet<_> = model.schedules.values().chain([&model.observers])
                .flat_map(|graph| graph.nodes.iter().map(|node| node.origin))
                .filter(|name| !name.is_empty()).collect();
            egui::ComboBox::from_id_salt("flow_plugin")
                .selected_text(self.plugin.as_deref().unwrap_or("Todos os plugins"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.plugin, None, "Todos os plugins");
                    for origin in origins {
                        ui.selectable_value(&mut self.plugin, Some(origin.into()), origin);
                    }
                });
            if self.view != View::Observers && let Some(graph) = model.schedules.get(&self.schedule) {
                let sets: Vec<_> = graph.nodes.iter().filter(|node| node.kind == NodeKind::Set).collect();
                let current = sets.iter().find(|node| Some(&node.id) == self.set.as_ref());
                egui::ComboBox::from_id_salt("flow_set")
                    .selected_text(current.map_or("Todos os sets", |node| short_name(&node.name)))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.set, None, "Todos os sets");
                        for node in sets {
                            ui.selectable_value(&mut self.set, Some(node.id.clone()), short_name(&node.name))
                                .on_hover_text(&node.name);
                        }
                    });
            }
            if self.view != View::Observers {
                ui.checkbox(&mut self.show_common, "Leituras comuns")
                    .on_hover_text("Mostra leituras compartilhadas de Transform, Time e AssetServer quando registradas pelo Bevy.");
            }
        });
    }

    fn visible(&self, model: &Model) -> Graph {
        let matches_plugin = |node: &Node| {
            self.plugin
                .as_ref()
                .is_none_or(|plugin| plugin == node.origin)
        };
        if self.view == View::Observers {
            let listeners: BTreeSet<_> = model
                .observers
                .nodes
                .iter()
                .enumerate()
                .filter_map(|(index, node)| {
                    (node.kind == NodeKind::Observer && matches_plugin(node)).then_some(index)
                })
                .collect();
            let events: BTreeSet<_> = model
                .observers
                .edges
                .iter()
                .filter_map(|edge| listeners.contains(&edge.to).then_some(edge.from))
                .collect();
            return model
                .observers
                .filtered(|index, _| listeners.contains(&index) || events.contains(&index));
        }
        let Some(graph) = model.schedules.get(&self.schedule) else {
            return Graph::default();
        };
        let member_filter = self
            .set
            .as_ref()
            .and_then(|id| graph.nodes.iter().position(|node| &node.id == id))
            .map(|index| graph.members_of(index));
        let in_set = |index: usize| {
            member_filter
                .as_ref()
                .is_none_or(|members| members.contains(&index))
        };
        let matching: Vec<_> = graph
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| (in_set(index) && matches_plugin(node)).then_some(index))
            .collect();
        let included = graph.set_ancestors(matching);
        let visible = graph.filtered(|index, _| included.contains(&index) && in_set(index));
        if self.view == View::Access {
            visible.accesses(self.show_common)
        } else {
            visible
        }
    }

    fn prefix(&self) -> String {
        if self.view == View::Observers {
            "Observers/".into()
        } else {
            format!("{:?}/{}/", self.view, self.schedule)
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui, graph: &Graph) {
        let size = ui.available_size().max(Vec2::ZERO);
        if size.x < 12.0 || size.y < 12.0 {
            return;
        }
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, theme::CARD_RADIUS, DEEP);
        if response.clicked_by(egui::PointerButton::Primary) {
            self.selected = None;
        }
        if response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Middle)
        {
            self.camera.offset += response.drag_delta();
        }
        if let Some(pointer) = ui
            .input(|input| input.pointer.hover_pos())
            .filter(|point| rect.contains(*point))
        {
            let wheel = ui.input(|input| input.smooth_scroll_delta.y);
            let current = self.camera.zoom;
            let zoom = (current * (wheel * 0.003).exp()).clamp(0.25, 2.5);
            let local = pointer - rect.min;
            self.camera.offset = local - (local - self.camera.offset) * (zoom / current);
            self.camera.zoom = zoom;
        }
        grid(&painter, rect.shrink(5.0), self.camera);
        if graph.nodes.is_empty() {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "Nenhum nó para os filtros escolhidos",
                FontId::proportional(13.0),
                SECONDARY,
            );
            return;
        }
        let prefix = self.prefix();
        let origin = rect.min + self.camera.offset;
        let zoom = self.camera.zoom;
        let positions = default_positions(graph, self.view);
        let mut nodes = Vec::with_capacity(graph.nodes.len());
        for (node, offset) in graph.nodes.iter().zip(positions) {
            let key = format!("{prefix}{}", node.id);
            let mut pos = self.layout.get(&key).unwrap_or(offset);
            let mut bounds = Rect::from_min_size(origin + pos.to_vec2() * zoom, NODE_SIZE * zoom);
            let mut highlighted = false;
            if bounds.intersects(rect) {
                let response = ui.interact(
                    bounds.intersect(rect),
                    ui.id().with(&key),
                    Sense::click_and_drag(),
                );
                if response.clicked() || response.drag_started_by(egui::PointerButton::Primary) {
                    self.selected = Some(key.clone());
                }
                if response.dragged_by(egui::PointerButton::Primary) {
                    let old_min = bounds.min;
                    pos += response.drag_delta() / zoom;
                    pos.x = pos.x.clamp(-999_999.0, 999_999.0);
                    pos.y = pos.y.clamp(-999_999.0, 999_999.0);
                    self.layout.set(key.clone(), pos);
                    bounds = bounds.translate(origin + pos.to_vec2() * zoom - old_min);
                }
                highlighted = response.hovered() || response.dragged();
                response.on_hover_text(tooltip(node, self.show_common));
            }
            nodes.push((bounds, highlighted, self.selected.as_ref() == Some(&key)));
        }
        let mut ports = Vec::with_capacity(graph.edges.len());
        for edge in &graph.edges {
            ports.push(draw_edge(
                &painter,
                edge,
                nodes[edge.from].0,
                nodes[edge.to].0,
                zoom,
            ));
        }
        for (node, (bounds, hovered, selected)) in graph.nodes.iter().zip(nodes) {
            if bounds.intersects(rect) {
                draw_node(
                    &painter,
                    node,
                    bounds,
                    zoom,
                    hovered,
                    selected,
                    self.show_common,
                );
            }
        }
        for (start, end, color) in ports {
            for port in [start, end] {
                painter.circle_filled(port, (3.0 * zoom).max(1.2), color);
            }
        }
    }
}

#[derive(AppLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FlowCapture;

pub(crate) fn install_capture(app: &mut App) {
    let mut capture = SubApp::new();
    let mut last = None;
    // The active schedule is temporarily removed from its World while it runs.
    // SubApp extraction sees the full registry after the main schedule returns.
    capture.set_extract(move |world, _| {
        let now = Instant::now();
        if last.is_some_and(|last| now.duration_since(last) < Duration::from_millis(500)) {
            return;
        }
        let model = Model::inspect(world);
        *world.resource_mut::<Model>() = model;
        last = Some(now);
    });
    app.insert_sub_app(FlowCapture, capture);
}

fn default_positions(graph: &Graph, view: View) -> Vec<Pos2> {
    let columns = match view {
        View::Access | View::Observers => graph
            .nodes
            .iter()
            .map(|node| usize::from(matches!(node.kind, NodeKind::Observer | NodeKind::Data)))
            .collect(),
        View::Order => {
            let mut incoming = vec![0; graph.nodes.len()];
            let mut children = vec![Vec::new(); graph.nodes.len()];
            let mut columns = vec![0; graph.nodes.len()];
            for edge in graph
                .edges
                .iter()
                .filter(|edge| edge.kind == EdgeKind::Order)
            {
                incoming[edge.to] += 1;
                children[edge.from].push(edge.to);
            }
            let mut ready: VecDeque<_> = incoming
                .iter()
                .enumerate()
                .filter_map(|(index, &count)| (count == 0).then_some(index))
                .collect();
            while let Some(from) = ready.pop_front() {
                for &to in &children[from] {
                    columns[to] = columns[to].max(columns[from] + 1);
                    incoming[to] -= 1;
                    if incoming[to] == 0 {
                        ready.push_back(to);
                    }
                }
            }
            columns
        }
    };
    let mut rows: BTreeMap<_, usize> = BTreeMap::new();
    let mut indices: Vec<_> = (0..graph.nodes.len()).collect();
    indices.sort_by_key(|&index| {
        (
            graph.nodes[index].kind != NodeKind::Set,
            &graph.nodes[index].name,
        )
    });
    let mut output = vec![Pos2::ZERO; graph.nodes.len()];
    for index in indices {
        let col = columns[index];
        let row = rows.entry(col).or_default();
        output[index] = pos2(20.0 + col as f32 * 278.0, 18.0 + *row as f32 * 82.0);
        *row += 1;
    }
    output
}

fn grid(painter: &egui::Painter, rect: Rect, camera: Camera) {
    let step = (24.0 * camera.zoom).max(12.0);
    let mut x = rect.left() + camera.offset.x.rem_euclid(step);
    while x < rect.right() {
        let mut y = rect.top() + camera.offset.y.rem_euclid(step);
        while y < rect.bottom() {
            painter.circle_filled(pos2(x, y), 0.65, BORDER);
            y += step;
        }
        x += step;
    }
}

fn draw_edge(
    painter: &egui::Painter,
    edge: &Edge,
    from: Rect,
    to: Rect,
    zoom: f32,
) -> (Pos2, Pos2, Color32) {
    let color = match edge.kind {
        EdgeKind::Order => RUST,
        EdgeKind::Member => MEMBERSHIP,
        EdgeKind::Read => READ,
        EdgeKind::Write => WRITE,
        EdgeKind::Listen => LISTEN,
    };
    let mut start_dir = 1.0;
    let mut end_dir = -1.0;
    if (from.center().x - to.center().x).abs() < 60.0 * zoom {
        end_dir = 1.0;
    } else if from.center().x > to.center().x {
        start_dir = -1.0;
        end_dir = 1.0;
    }
    let start = from.center() + vec2(start_dir * from.width() * 0.5, 0.0);
    let end = to.center() + vec2(end_dir * to.width() * 0.5, 0.0);
    let bend = (start.distance(end) * 0.42).clamp(25.0 * zoom, 145.0 * zoom);
    let points = [
        start,
        start + vec2(start_dir * bend, 0.0),
        end + vec2(end_dir * bend, 0.0),
        end,
    ];
    let stroke = Stroke::new((1.5 * zoom).max(0.85), color);
    painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
        points,
        false,
        Color32::TRANSPARENT,
        stroke,
    ));
    let side = 3.0 * zoom;
    let tip = end + vec2(end_dir * 4.0 * zoom, 0.0);
    let tail = end + vec2(end_dir * 10.0 * zoom, 0.0);
    painter.add(egui::Shape::convex_polygon(
        vec![tip, tail + vec2(0.0, side), tail - vec2(0.0, side)],
        color,
        Stroke::NONE,
    ));
    (start, end, color)
}

fn draw_node(
    painter: &egui::Painter,
    node: &Node,
    bounds: Rect,
    zoom: f32,
    hovered: bool,
    selected: bool,
    show_common: bool,
) {
    painter.rect(
        bounds,
        (f32::from(theme::CARD_RADIUS) * zoom).round(),
        if hovered { HOVER } else { SURFACE },
        if selected {
            Stroke::new(1.5, RUST)
        } else {
            Stroke::NONE
        },
        egui::StrokeKind::Inside,
    );
    let start = bounds.min + vec2(10.0, 8.0) * zoom;
    let label = ellipsis(short_name(&node.name), 28);
    painter.text(
        start,
        Align2::LEFT_TOP,
        label,
        FontId::monospace(12.0 * zoom),
        TEXT,
    );
    let visible = || {
        node.access
            .iter()
            .filter(|a| show_common || !a.hidden_by_default())
    };
    let info = visible()
        .next()
        .map(|a| short_name(&a.path))
        .filter(|_| node.kind == NodeKind::System)
        .unwrap_or_else(|| {
            if node.origin.is_empty() {
                node.kind.label()
            } else {
                node.origin
            }
        });
    painter.text(
        start + vec2(0.0, 23.0) * zoom,
        Align2::LEFT_TOP,
        ellipsis(
            info,
            if node.kind == NodeKind::System {
                19
            } else {
                30
            },
        ),
        FontId::monospace(10.0 * zoom),
        SECONDARY,
    );
    if node.kind == NodeKind::System {
        let reads = visible().filter(|a| a.mark() == "R").count();
        let writes = visible().filter(|a| a.mark() == "W").count();
        let badge = painter.layout_no_wrap(
            format!("R{reads} · W{writes}"),
            FontId::monospace(9.0 * zoom),
            TEXT,
        );
        let size = badge.size();
        let rect = Rect::from_min_size(
            bounds.right_top() + vec2(-17.0 * zoom - size.x, 30.0 * zoom),
            size + vec2(12.0, 5.0) * zoom,
        );
        painter.rect_filled(rect, theme::PILL_RADIUS, RUST_SOFT);
        painter.galley(rect.center() - size * 0.5, badge, TEXT);
    }
}

fn tooltip(node: &Node, show_common: bool) -> String {
    let mut text = format!("{}\n{}", node.kind.label(), node.name);
    if !node.origin.is_empty() {
        text.push_str(&format!("\nPlugin: {}", node.origin));
    }
    if !node.info.is_empty() {
        text.push_str(&format!("\n{}", node.info));
    }
    for access in node
        .access
        .iter()
        .filter(|access| show_common || !access.hidden_by_default())
    {
        text.push_str(&format!("\n{}  {}", access.mark(), access.path));
    }
    text
}

fn short_name(name: &str) -> &str {
    if name.contains(['<', '(', ' ']) {
        name
    } else {
        name.rsplit("::").next().unwrap_or(name)
    }
}

fn ellipsis(text: &str, max: usize) -> String {
    let mut chars = text.chars();
    let mut shortened: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        shortened.push('…');
    }
    shortened
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component)]
    struct Meter;

    #[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum Group {
        Parent,
        Nested,
    }

    #[derive(EntityEvent)]
    struct ChangedMeter {
        entity: Entity,
    }

    fn write_meter(_: Query<&mut Meter>) {}
    fn read_meter(_: Query<(&Meter, &Transform)>) {}
    fn update_other(_: Query<&Transform>) {}
    fn watch_meter(_: On<ChangedMeter>) {}
    fn at_end() {}

    #[test]
    fn captures_the_schedule_orders_sets_and_observers_and_filters_by_origin() {
        let mut app = App::new();
        let mut origins = Origins::default();
        origins.record(&mut app, "Bevy / App");
        let target = app.world_mut().spawn_empty().id();
        app.configure_sets(Update, Group::Nested.in_set(Group::Parent))
            .add_systems(
                Update,
                (write_meter, read_meter).chain().in_set(Group::Nested),
            )
            .add_systems(Last, at_end);
        app.world_mut()
            .spawn(Observer::new(watch_meter).with_entity(target));
        origins.record(&mut app, "GamePlugin");
        app.add_systems(Update, update_other);
        origins.record(&mut app, "OtherPlugin");
        app.insert_resource(origins).init_resource::<Model>();
        install_capture(&mut app);
        app.update();

        let model = app.world().resource::<Model>();
        assert!(model.schedules.get("Last").is_some_and(|graph| {
            graph
                .nodes
                .iter()
                .any(|node| node.name.ends_with("::at_end"))
        }));
        let temp = tempfile::tempdir().unwrap();
        let mut panel = FlowGraph::new(
            temp.path().join(".editor/graph_layout.ron"),
            Some("GamePlugin"),
        );
        let graph = panel.visible(model);
        assert!(
            graph
                .nodes
                .iter()
                .all(|node| !node.name.ends_with("::update_other"))
        );
        let write = graph
            .nodes
            .iter()
            .position(|node| node.name.ends_with("::write_meter"))
            .unwrap();
        let read = graph
            .nodes
            .iter()
            .position(|node| node.name.ends_with("::read_meter"))
            .unwrap();
        assert!(
            graph
                .edges
                .iter()
                .any(|edge| edge.from == write && edge.to == read && edge.kind == EdgeKind::Order)
        );
        let parent = graph
            .nodes
            .iter()
            .position(|node| node.kind == NodeKind::Set && node.name.ends_with("Parent"))
            .unwrap();
        assert!(graph.members_of(parent).contains(&read));
        let positions = default_positions(&graph, View::Order);
        assert!(positions[write].x < positions[read].x);
        panel.set = Some(graph.nodes[parent].id.clone());

        panel.view = View::Access;
        let queries = panel.visible(model);
        assert!(
            queries
                .nodes
                .iter()
                .any(|node| node.kind == NodeKind::Data && node.name.ends_with("::Meter"))
        );
        assert!(
            !queries
                .nodes
                .iter()
                .any(|node| node.kind == NodeKind::Data && node.name.ends_with("::Transform"))
        );
        assert!(
            queries
                .edges
                .iter()
                .any(|edge| edge.kind == EdgeKind::Write)
        );
        assert!(queries.edges.iter().any(|edge| edge.kind == EdgeKind::Read));
        panel.show_common = true;
        assert!(
            panel
                .visible(model)
                .nodes
                .iter()
                .any(|node| node.kind == NodeKind::Data && node.name.ends_with("::Transform"))
        );

        panel.view = View::Observers;
        let observers = panel.visible(model);
        assert_eq!(observers.edges.len(), 1);
        let edge = observers.edges[0];
        assert_eq!(edge.kind, EdgeKind::Listen);
        assert!(observers.nodes[edge.from].name.ends_with("::ChangedMeter"));
        assert!(observers.nodes[edge.to].name.ends_with("::watch_meter"));
        assert!(observers.nodes[edge.to].info.contains("Alvos:"));
    }
}
