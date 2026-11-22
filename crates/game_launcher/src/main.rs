use std::{env, path::PathBuf, time::Duration};

use bevy::{
    app::ScheduleRunnerPlugin, asset::io::AssetSourceBuilder, log::LogPlugin, prelude::*,
    world_serialization::WorldSerializationPlugin,
};
use game::{GamePlugin, Health};

fn main() -> AppExit {
    let root = env::var_os("BEVY_ASSET_ROOT")
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok());
    let Some(root) = root else {
        eprintln!("Falha: não foi possível localizar o diretório de assets.");
        return AppExit::error();
    };
    let assets = root.join("assets");
    let scenes = root.join("scenes");

    App::new()
        .register_asset_source(
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
            GamePlugin,
        ))
        .add_systems(Last, verify_demo)
        .run()
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
