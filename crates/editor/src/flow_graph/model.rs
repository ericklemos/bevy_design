use std::collections::{BTreeMap, BTreeSet, HashMap};

use bevy::{
    ecs::{
        component::ComponentId,
        query::ComponentAccessKind,
        schedule::{InternedScheduleLabel, NodeId, Schedule, Schedules},
        system::SystemAccess,
    },
    prelude::*,
};

const RUNTIME: &str = "Origem não registrada";

/// Registration provenance is measured at the plugin boundary, not guessed from Rust paths.
#[derive(Resource, Default)]
pub(crate) struct Origins {
    schedules: HashMap<(InternedScheduleLabel, NodeId), &'static str>,
    observers: HashMap<Entity, &'static str>,
}

impl Origins {
    pub(crate) fn record(&mut self, app: &mut App, plugin: &'static str) {
        let world = app.world_mut();
        if let Some(schedules) = world.get_resource::<Schedules>() {
            for (_, schedule) in schedules.iter() {
                let graph = schedule.graph();
                let ids = graph
                    .systems
                    .iter()
                    .map(|(id, _, _)| NodeId::System(id))
                    .chain(graph.system_sets.iter().map(|(id, _, _)| NodeId::Set(id)));
                for id in ids {
                    self.schedules
                        .entry((schedule.label(), id))
                        .or_insert(plugin);
                }
            }
        }
        let mut observers = world.query_filtered::<Entity, With<Observer>>();
        for entity in observers.iter(world) {
            self.observers.entry(entity).or_insert(plugin);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NodeKind {
    System,
    Set,
    Data,
    Event,
    Observer,
}

impl NodeKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::System => "Sistema",
            Self::Set => "SystemSet",
            Self::Data => "Tipo ECS",
            Self::Event => "Evento",
            Self::Observer => "Observer",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EdgeKind {
    Order,
    Member,
    Read,
    Write,
    Listen,
}

#[derive(Debug, Clone)]
pub(crate) struct Access {
    pub(crate) path: String,
    pub(crate) kind: ComponentAccessKind,
}

impl Access {
    pub(crate) fn hidden_by_default(&self) -> bool {
        if !matches!(self.kind, ComponentAccessKind::Shared(_)) {
            return false;
        }
        (self.path.starts_with("bevy_transform::") && self.path.ends_with("::Transform"))
            || self.path.starts_with("bevy_time::time::Time<")
            || self.path == "bevy_time::time::Time"
            || (self.path.starts_with("bevy_asset::") && self.path.ends_with("::AssetServer"))
            || (self.path.starts_with("bevy_ecs::") && self.path.ends_with("::Commands"))
    }

    pub(crate) fn mark(&self) -> &'static str {
        match self.kind {
            ComponentAccessKind::Shared(_) => "R",
            ComponentAccessKind::Exclusive(_) => "W",
            ComponentAccessKind::Archetypal(_) => "Filtro",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Node {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) kind: NodeKind,
    pub(crate) origin: &'static str,
    pub(crate) info: String,
    pub(crate) access: Vec<Access>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Edge {
    pub(crate) from: usize,
    pub(crate) to: usize,
    pub(crate) kind: EdgeKind,
}

#[derive(Default, Debug)]
pub(crate) struct Graph {
    pub(crate) nodes: Vec<Node>,
    pub(crate) edges: Vec<Edge>,
}

impl Graph {
    pub(crate) fn filtered(&self, mut include: impl FnMut(usize, &Node) -> bool) -> Self {
        let mut mapping = vec![None; self.nodes.len()];
        let mut result = Self::default();
        for (index, node) in self.nodes.iter().enumerate() {
            if include(index, node) {
                mapping[index] = Some(result.nodes.len());
                result.nodes.push(node.clone());
            }
        }
        result.edges.extend(self.edges.iter().filter_map(|edge| {
            Some(Edge {
                from: mapping[edge.from]?,
                to: mapping[edge.to]?,
                kind: edge.kind,
            })
        }));
        result
    }

    pub(crate) fn members_of(&self, set: usize) -> BTreeSet<usize> {
        traverse([set], &self.edges, false, EdgeKind::Member)
    }

    pub(crate) fn set_ancestors(&self, starts: impl IntoIterator<Item = usize>) -> BTreeSet<usize> {
        traverse(starts, &self.edges, true, EdgeKind::Member)
    }

    pub(crate) fn accesses(&self, show_common: bool) -> Self {
        let mut result = self.filtered(|_, node| node.kind == NodeKind::System);
        result.edges.clear();
        let mut types = BTreeMap::new();
        for from in 0..result.nodes.len() {
            let accesses = result.nodes[from].access.clone();
            for access in accesses {
                if (!show_common && access.hidden_by_default())
                    || matches!(access.kind, ComponentAccessKind::Archetypal(_))
                {
                    continue;
                }
                let to = *types.entry(access.path.clone()).or_insert_with(|| {
                    let index = result.nodes.len();
                    result.nodes.push(Node {
                        id: format!("data::{}", access.path),
                        name: access.path,
                        kind: NodeKind::Data,
                        origin: "",
                        info: String::new(),
                        access: Vec::new(),
                    });
                    index
                });
                if matches!(access.kind, ComponentAccessKind::Exclusive(_)) {
                    result.edges.push(Edge {
                        from,
                        to,
                        kind: EdgeKind::Write,
                    });
                } else {
                    result.edges.push(Edge {
                        from: to,
                        to: from,
                        kind: EdgeKind::Read,
                    });
                }
            }
        }
        result
    }
}

fn traverse(
    starts: impl IntoIterator<Item = usize>,
    edges: &[Edge],
    reverse: bool,
    kind: EdgeKind,
) -> BTreeSet<usize> {
    let mut neighbors = HashMap::<usize, Vec<usize>>::new();
    for edge in edges.iter().filter(|edge| edge.kind == kind) {
        let (from, to) = if reverse {
            (edge.to, edge.from)
        } else {
            (edge.from, edge.to)
        };
        neighbors.entry(from).or_default().push(to);
    }
    let mut pending: Vec<_> = starts.into_iter().collect();
    let mut visited = BTreeSet::new();
    while let Some(index) = pending.pop() {
        if visited.insert(index)
            && let Some(items) = neighbors.get(&index)
        {
            pending.extend(items.iter().copied());
        }
    }
    visited
}

#[derive(Resource, Default)]
pub(crate) struct Model {
    pub(crate) schedules: BTreeMap<String, Graph>,
    pub(crate) observers: Graph,
}

impl Model {
    pub(crate) fn inspect(world: &mut World) -> Self {
        let mut query = world.query::<(Entity, &Observer)>();
        let origins = world.resource::<Origins>();
        let mut schedules = BTreeMap::new();
        if let Some(registered) = world.get_resource::<Schedules>() {
            for (label, schedule) in registered.iter() {
                let label = format!("{label:?}");
                let mut name = label.clone();
                let mut duplicate = 2;
                while schedules.contains_key(&name) {
                    name = format!("{label} #{duplicate}");
                    duplicate += 1;
                }
                schedules.insert(name, inspect_schedule(world, schedule, origins));
            }
        }
        let observers = inspect_observers(world, query.iter(world).collect(), origins);
        Self {
            schedules,
            observers,
        }
    }
}

fn inspect_schedule(world: &World, schedule: &Schedule, origins: &Origins) -> Graph {
    let graph = schedule.graph();
    let dependencies = graph.dependency().graph();
    let hierarchy = graph.hierarchy().graph();
    let mut entries = BTreeMap::new();
    let sort_name = |name: String, kind: NodeKind, access: Vec<Access>, info: String| {
        (name, kind, access, info)
    };
    // Systems can still be registered when the schedule has not run yet.
    for (id, system, _) in graph.systems.iter() {
        entries.insert(
            NodeId::System(id),
            sort_name(
                system.name().to_string(),
                NodeKind::System,
                Vec::new(),
                "Acessos ainda não inicializados".into(),
            ),
        );
    }
    if let Ok(systems) = schedule.systems_with_access() {
        for (id, system) in systems {
            let name = system.system().name().to_string();
            // Automatically inserted sync points are not present in the authoring graph.
            if !dependencies.contains_node(NodeId::System(id))
                && !hierarchy.contains_node(NodeId::System(id))
                && !entries.contains_key(&NodeId::System(id))
            {
                continue;
            }
            let (access, info) = inspect_access(world, system.access());
            entries.insert(
                NodeId::System(id),
                sort_name(name, NodeKind::System, access, info),
            );
        }
    }
    for (id, set, _) in graph.system_sets.iter() {
        let id = NodeId::Set(id);
        let used_for_order = dependencies.edges(id).next().is_some()
            || dependencies
                .edges_directed(id, bevy::ecs::schedule::graph::Direction::Incoming)
                .next()
                .is_some();
        if used_for_order || set.system_type().is_none() {
            let name = if set.is_anonymous() {
                let names = hierarchy
                    .edges(id)
                    .filter_map(|(_, member)| entries.get(&member))
                    .map(|(name, _, _, _)| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("Grupo anônimo ({names})")
            } else {
                format!("{set:?}")
            };
            entries.insert(
                id,
                sort_name(
                    name,
                    NodeKind::Set,
                    Vec::new(),
                    "Agrupamento de sistemas".into(),
                ),
            );
        }
    }
    let mut counters = HashMap::<String, usize>::new();
    let mut indices = HashMap::new();
    let mut result = Graph::default();
    for (id, (name, kind, access, info)) in entries {
        let key = format!("{}::{name}", kind.label());
        let ordinal = counters.entry(key.clone()).or_default();
        *ordinal += 1;
        indices.insert(id, result.nodes.len());
        result.nodes.push(Node {
            id: format!("{key}::{ordinal}"),
            name,
            kind,
            origin: origins
                .schedules
                .get(&(schedule.label(), id))
                .copied()
                .unwrap_or(RUNTIME),
            info,
            access,
        });
    }
    for (kind, edges) in [
        (EdgeKind::Order, dependencies),
        (EdgeKind::Member, hierarchy),
    ] {
        result
            .edges
            .extend(edges.all_edges().filter_map(|(from, to)| {
                Some(Edge {
                    from: *indices.get(&from)?,
                    to: *indices.get(&to)?,
                    kind,
                })
            }));
    }
    result
}

fn inspect_access(world: &World, access: &SystemAccess) -> (Vec<Access>, String) {
    match access {
        SystemAccess::None => (Vec::new(), String::new()),
        SystemAccess::Exclusive => (Vec::new(), "Acesso exclusivo ao World".into()),
        SystemAccess::Shared(access) => {
            let Ok(iter) = access.combined_access().try_iter_access() else {
                return (Vec::new(), "Acesso amplo ao World (não enumerável)".into());
            };
            let mut items: Vec<_> = iter
                .map(|kind| Access {
                    path: component_name(world, *kind.index()),
                    kind,
                })
                .collect();
            items.sort_by(|a, b| a.path.cmp(&b.path));
            (items, String::new())
        }
    }
}

fn component_name(world: &World, id: ComponentId) -> String {
    world.components().get_info(id).map_or_else(
        || format!("Componente {id:?}"),
        |info| info.name().to_string(),
    )
}

fn event_name(world: &World, id: ComponentId) -> String {
    let name = component_name(world, id);
    // Bevy represents typed events as component IDs of a generic wrapper.
    name.strip_prefix("bevy_ecs::event::EventWrapperComponent<")
        .and_then(|inner| inner.strip_suffix('>'))
        .map(str::to_owned)
        .unwrap_or(name)
}

fn inspect_observers(
    world: &World,
    mut observers: Vec<(Entity, &Observer)>,
    origins: &Origins,
) -> Graph {
    observers.sort_by_key(|(entity, _)| *entity);
    let mut counters = HashMap::<String, usize>::new();
    let mut events = HashMap::new();
    let mut result = Graph::default();
    for (entity, observer) in observers {
        let descriptor = observer.descriptor();
        let name = observer.system_name().to_string();
        let mut event_paths: Vec<_> = descriptor
            .event_keys()
            .iter()
            .map(|event| {
                (
                    event.component_id(),
                    event_name(world, event.component_id()),
                )
            })
            .collect();
        event_paths.sort_by(|a, b| a.1.cmp(&b.1));
        let signature = event_paths
            .iter()
            .map(|(_, name)| name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let key = format!("{name}::{signature}");
        let ordinal = counters.entry(key.clone()).or_default();
        *ordinal += 1;
        let to = result.nodes.len();
        let mut details = Vec::new();
        if !descriptor.entities().is_empty() {
            details.push(format!("Alvos: {:?}", descriptor.entities()));
        }
        if !descriptor.components().is_empty() {
            let names = descriptor
                .components()
                .iter()
                .map(|&id| component_name(world, id))
                .collect::<Vec<_>>()
                .join(", ");
            details.push(format!("Componentes observados: {names}"));
        }
        if details.is_empty() {
            details.push("Registro global, sem filtro de entidade ou componente".into());
        }
        result.nodes.push(Node {
            id: format!("observer::{key}::{ordinal}"),
            name,
            kind: NodeKind::Observer,
            origin: origins.observers.get(&entity).copied().unwrap_or(RUNTIME),
            info: details.join("\n"),
            access: Vec::new(),
        });
        for (id, name) in event_paths {
            let from = *events.entry(id).or_insert_with(|| {
                let index = result.nodes.len();
                result.nodes.push(Node {
                    id: format!("event::{name}"),
                    name,
                    kind: NodeKind::Event,
                    origin: "",
                    info: "Disparo registrado pela assinatura; emissor não inferido".into(),
                    access: Vec::new(),
                });
                index
            });
            result.edges.push(Edge {
                from,
                to,
                kind: EdgeKind::Listen,
            });
        }
    }
    result
}
