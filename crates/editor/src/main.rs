mod asset_browser;
mod config;
mod console;
mod flow_graph;
mod gizmo;
mod icons;
mod inspector;
mod session;
mod theme;
mod ui;
mod viewport;

use std::{env, path::PathBuf};

use bevy::{asset::io::AssetSourceBuilder, prelude::*};
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use game::{GamePlugin, GameSimulation, PlayerConfig};

#[derive(Component)]
struct EditorOnly;

fn main() -> AppExit {
    let root = env::var_os("BEVY_ASSET_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().expect("diretório do projeto indisponível"));
    let root = std::path::absolute(root).expect("caminho do projeto inválido");
    let mut app = App::new();
    let mut origins = flow_graph::Origins::default();
    origins.record(&mut app, "Bevy / App");
    app.register_asset_source(
        "scenes",
        AssetSourceBuilder::platform_default(&root.join("scenes").to_string_lossy(), None),
    )
    .add_plugins(
        DefaultPlugins
            .set(bevy::log::LogPlugin {
                custom_layer: console::log_layer,
                ..default()
            })
            .set(AssetPlugin {
                file_path: root.join("assets").to_string_lossy().into_owned(),
                watch_for_changes_override: Some(true),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Bevy".into(),
                    resolution: (1440, 900).into(),
                    ..default()
                }),
                ..default()
            }),
    );
    origins.record(&mut app, "DefaultPlugins (grupo)");
    app.insert_resource(GameSimulation(false))
        .insert_resource(config::ConfigEditor::new(
            root.join("assets").join(PlayerConfig::PATH),
        ))
        .add_plugins(GamePlugin);
    origins.record(&mut app, "GamePlugin");
    app.add_plugins(EguiPlugin::default());
    origins.record(&mut app, "EguiPlugin");
    app.init_resource::<session::Session>()
        .insert_resource(asset_browser::Index::new(root.clone()))
        .init_resource::<ui::Workspace>()
        .init_resource::<inspector::Inspector>()
        .init_resource::<flow_graph::Model>()
        .insert_resource(flow_graph::FlowGraph::new(
            root.join(".editor/graph_layout.ron"),
        ))
        .register_type::<Name>()
        .add_systems(Startup, viewport::setup)
        .add_systems(Update, (inspector::refresh, config::update))
        .add_systems(
            PostUpdate,
            viewport::update_preview
                .after(bevy::transform::TransformSystems::Propagate)
                .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
        )
        .add_systems(EguiPrimaryContextPass, ui::draw);
    origins.record(&mut app, "Editor");
    app.insert_resource(origins);
    flow_graph::install_capture(&mut app);
    app.run()
}
