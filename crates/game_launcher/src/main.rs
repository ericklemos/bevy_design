use std::{env, path::Path, time::Duration};

use bevy::{
    app::ScheduleRunnerPlugin, asset::io::AssetSourceBuilder, log::LogPlugin, prelude::*,
    world_serialization::WorldSerializationPlugin,
};
use engine_runtime::StartupScene;
use game::{GamePlugin, Health, PlayerConfig};
use projects::Project;

fn main() -> AppExit {
    let mut args: Vec<_> = env::args_os().skip(1).collect();
    let verify = args.last().is_some_and(|arg| arg == "--verify-demo");
    if verify {
        args.pop();
    }
    let root = match args.as_slice() {
        [] => env::var_os("BEVY_ASSET_ROOT").unwrap_or_else(|| ".".into()),
        [root] => root.clone(),
        _ => {
            eprintln!("Uso: game_launcher [pasta-do-projeto] [--verify-demo]");
            return AppExit::error();
        }
    };
    let project = match Project::open(Path::new(&root)) {
        Ok(project) => project,
        Err(error) => {
            eprintln!("Falha no projeto: {error}");
            return AppExit::error();
        }
    };
    let assets = project.assets();
    let scenes = project.scenes();
    let has_game = assets.join(PlayerConfig::PATH).is_file();
    if verify && !has_game {
        eprintln!(
            "Falha: a verificação da demo requer {}.",
            PlayerConfig::PATH
        );
        return AppExit::error();
    }

    let mut app = App::new();
    app.register_asset_source(
        "scenes",
        AssetSourceBuilder::platform_default(&scenes.to_string_lossy(), None),
    )
    .add_plugins((
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_millis(16))),
        LogPlugin::default(),
        AssetPlugin {
            file_path: assets.to_string_lossy().into_owned(),
            watch_for_changes_override: Some(true),
            ..default()
        },
        WorldSerializationPlugin,
        StartupScene(project.startup_scene().into()),
    ));
    app.register_type::<Name>().register_type::<Transform>();
    if has_game {
        app.add_plugins(GamePlugin);
    }
    if verify {
        app.add_systems(Last, verify_demo);
    }
    app.run()
}

fn verify_demo(players: Query<&Health>, time: Res<Time<Real>>, mut exit: MessageWriter<AppExit>) {
    if let Ok(health) = players.single()
        && health.max == 100.0
        && health.current == 90.0
    {
        println!("OK: level.scn.ron + player_config.ron + Observer (Health: 90/100)");
        exit.write(AppExit::Success);
    } else if time.elapsed_secs() >= 5.0 {
        eprintln!(
            "Falha: a cena, a configuração e o Observer não produziram Health 90/100 em 5 s."
        );
        exit.write(AppExit::error());
    }
}
