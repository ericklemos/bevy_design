mod asset_browser;
mod config;
mod console;
mod flow_graph;
mod gizmo;
mod icons;
mod inspector;
mod project_picker;
mod recent_projects;
mod session;
mod theme;
mod ui;
mod viewport;
mod welcome;

use std::{env, path::Path};

use bevy::{asset::io::AssetSourceBuilder, camera::CameraUpdateSystems, prelude::*};
use bevy_egui::{EguiPlugin, EguiPostUpdateSet, EguiPrimaryContextPass};
use engine_runtime::StartupScene;
use game::{GamePlugin, GameSimulation, PlayerConfig};
use projects::{MANIFEST_FILE, Project};

#[derive(Component)]
struct EditorOnly;

fn main() -> AppExit {
    let args: Vec<_> = env::args_os().skip(1).collect();
    let project = match args.as_slice() {
        [] => {
            if let Some(root) = env::var_os("BEVY_ASSET_ROOT") {
                Project::open(Path::new(&root))
            } else if Path::new(MANIFEST_FILE).exists() {
                Project::open(Path::new("."))
            } else {
                return welcome::run();
            }
        }
        [command, root] if command == "open" => Project::open(Path::new(root)),
        [command, root, name] if command == "new" => {
            let result = name
                .to_str()
                .ok_or_else(|| std::io::Error::other("o nome deve estar em UTF-8"))
                .and_then(|name| Project::create(Path::new(root), name));
            return match result {
                Ok(project) => {
                    println!("Projeto criado em {}", project.root().display());
                    AppExit::Success
                }
                Err(error) => project_error(error),
            };
        }
        _ => {
            eprintln!("Uso: editor [open <pasta> | new <nova-pasta> <nome>]");
            return AppExit::error();
        }
    };
    match project {
        Ok(project) => run_editor(project),
        Err(error) => project_error(error),
    }
}

fn project_error(error: std::io::Error) -> AppExit {
    eprintln!("Falha no projeto: {error}");
    AppExit::error()
}

fn run_editor(project: Project) -> AppExit {
    if let Err(error) =
        recent_projects::History::local().and_then(|history| history.record(&project))
    {
        eprintln!("Não foi possível atualizar os projetos recentes: {error}");
    }
    let root = project.root();
    let assets = project.assets();
    let config_path = assets.join(PlayerConfig::PATH);
    let has_game = config_path.is_file();
    let mut app = App::new();
    let mut origins = flow_graph::Origins::default();
    origins.record(&mut app, "Bevy / App");
    app.register_asset_source(
        "scenes",
        AssetSourceBuilder::platform_default(&project.scenes().to_string_lossy(), None),
    )
    .add_plugins(
        DefaultPlugins
            .set(bevy::log::LogPlugin {
                custom_layer: console::log_layer,
                ..default()
            })
            .set(AssetPlugin {
                file_path: assets.to_string_lossy().into_owned(),
                watch_for_changes_override: Some(true),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: format!("{} — Bevy Editor", project.name()),
                    resolution: (1440, 900).into(),
                    ..default()
                }),
                ..default()
            }),
    );
    origins.record(&mut app, "DefaultPlugins (grupo)");
    app.add_plugins(StartupScene(project.startup_scene().into()))
        .insert_resource(GameSimulation(false));
    origins.record(&mut app, "StartupScene");
    if has_game {
        app.add_plugins(GamePlugin);
        origins.record(&mut app, "GamePlugin");
        app.insert_resource(config::ConfigEditor::new(config_path))
            .add_systems(Update, config::update);
    }
    app.add_plugins(EguiPlugin::default());
    origins.record(&mut app, "EguiPlugin");
    app.add_plugins(icons::MaterialSymbols)
        .init_resource::<session::Session>()
        .insert_resource(asset_browser::Index::new(root.to_path_buf()))
        .init_resource::<ui::Workspace>()
        .init_resource::<inspector::Inspector>()
        .init_resource::<flow_graph::Model>()
        .insert_resource(flow_graph::FlowGraph::new(
            root.join(".editor/graph_layout.ron"),
            has_game.then_some("GamePlugin"),
        ))
        .register_type::<Name>()
        .register_type::<Transform>()
        .add_systems(Startup, viewport::setup)
        .add_systems(Update, inspector::refresh)
        .add_systems(
            PostUpdate,
            (
                viewport::update_preview
                    .after(bevy::transform::TransformSystems::Propagate)
                    .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
                viewport::resize_target
                    .after(EguiPostUpdateSet::EndPass)
                    .before(CameraUpdateSystems),
            ),
        )
        .add_systems(EguiPrimaryContextPass, ui::draw);
    origins.record(&mut app, "Editor");
    app.insert_resource(origins);
    flow_graph::install_capture(&mut app);
    app.run()
}
