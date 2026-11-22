use std::{env, fs, io, path::PathBuf};

use bevy::{asset::io::AssetSourceBuilder, prelude::*, world_serialization::DynamicWorldRoot};
use serde::Deserialize;

const MANIFEST_FILE: &str = "project.ron";

#[derive(Deserialize)]
struct Manifest {
    version: u32,
    name: String,
    startup_scene: String,
}

fn main() -> AppExit {
    match run() {
        Ok(exit) => exit,
        Err(error) => {
            eprintln!("Não foi possível iniciar o jogo: {error}");
            AppExit::error()
        }
    }
}

fn run() -> Result<AppExit, Box<dyn std::error::Error>> {
    let root = project_root()?;
    let manifest: Manifest = ron::de::from_bytes(&fs::read(root.join(MANIFEST_FILE))?)?;
    if manifest.version != 1 {
        return Err(format!("Versão de projeto não suportada: {}", manifest.version).into());
    }
    let scene = &manifest.startup_scene;
    if !scene.ends_with(".scn.ron")
        || scene.contains(['\\', ':', '#'])
        || scene.chars().any(char::is_control)
        || scene.split('/').any(|part| matches!(part, "" | "." | ".."))
        || !root.join("scenes").join(scene).is_file()
    {
        return Err(format!("Cena inicial inválida ou ausente: {scene}").into());
    }
    let scene = format!("scenes://{scene}");
    let assets = root.join("assets");
    let scenes = root.join("scenes");
    let assets = assets
        .to_str()
        .ok_or("Caminho de assets não está em UTF-8")?;
    let scenes = scenes
        .to_str()
        .ok_or("Caminho de scenes não está em UTF-8")?;

    Ok(App::new()
        .register_asset_source("scenes", AssetSourceBuilder::platform_default(scenes, None))
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: assets.into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: manifest.name,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .register_type::<Name>()
        .register_type::<Transform>()
        .add_systems(
            Startup,
            move |mut commands: Commands, assets: Res<AssetServer>| {
                commands.spawn((
                    DynamicWorldRoot(assets.load(scene.clone())),
                    Transform::IDENTITY,
                ));
                // Câmera inicial. Substitua-a pela câmera do seu jogo.
                commands.spawn((
                    Camera3d::default(),
                    Transform::from_xyz(0.0, 3.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
                ));
            },
        )
        .run())
}

fn project_root() -> io::Result<PathBuf> {
    let exe = env::current_exe()?;
    // Pacote de distribuição: dados ao lado do binário. Cargo: pasta atual
    // ou um ancestral do binário em target/debug ou target/release.
    exe.parent()
        .into_iter()
        .map(PathBuf::from)
        .chain(env::current_dir().ok())
        .chain(exe.ancestors().skip(2).map(PathBuf::from))
        .find(|dir| dir.join(MANIFEST_FILE).is_file())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "project.ron não encontrado"))
}
