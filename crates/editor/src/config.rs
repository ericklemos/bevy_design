use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use bevy::prelude::*;
use bevy_egui::egui;
use game::PlayerConfig;

use crate::inspector;

const DEBOUNCE: Duration = Duration::from_millis(350);

#[derive(Resource)]
pub(crate) struct ConfigEditor {
    path: PathBuf,
    draft: Option<PlayerConfig>,
    last_seen: Option<PlayerConfig>,
    awaiting_reload: Option<PlayerConfig>,
    original: Option<Vec<u8>>,
    due: Option<Instant>,
    error: Option<String>,
}

impl ConfigEditor {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            draft: None,
            last_seen: None,
            awaiting_reload: None,
            original: None,
            due: None,
            error: None,
        }
    }

    fn synchronize(&mut self, asset: &PlayerConfig) {
        if self.due.is_some() || self.error.is_some() {
            return;
        }
        if self.awaiting_reload.as_ref() != Some(asset) && self.last_seen.as_ref() == Some(asset) {
            return;
        }
        // Do not take the old loaded asset as the baseline after writing a new one.
        // Read the same bytes that justify the presented values; an external
        // change between load and read must not be accepted as the write baseline.
        if let Ok(bytes) = fs::read(&self.path)
            && ron::de::from_bytes::<PlayerConfig>(&bytes).is_ok_and(|config| &config == asset)
        {
            self.awaiting_reload = None;
            self.original = Some(bytes);
            self.last_seen = Some(asset.clone());
            self.draft = Some(asset.clone());
        }
    }

    fn write_draft(&mut self) -> io::Result<()> {
        let (Some(draft), Some(original)) = (&self.draft, &self.original) else {
            return Ok(());
        };
        if ![draft.vida_maxima, draft.velocidade, draft.gravidade]
            .into_iter()
            .all(f32::is_finite)
            || draft.vida_maxima < 0.0
            || draft.velocidade < 0.0
        {
            return Err(io::Error::other(
                "use valores finitos e não negativos para vida e velocidade",
            ));
        }
        if self.last_seen.as_ref() == Some(draft)
            && self.awaiting_reload.is_none()
            && fs::read(&self.path)? == *original
        {
            return Ok(());
        }
        let mut contents = ron::ser::to_string_pretty(draft, ron::ser::PrettyConfig::default())
            .map_err(io::Error::other)?
            .into_bytes();
        contents.push(b'\n');
        write_if_unchanged(&self.path, original, &contents)?;
        self.original = Some(contents);
        self.awaiting_reload = Some(draft.clone());
        Ok(())
    }

    fn flush(&mut self) {
        self.due = None;
        self.error = self.write_draft().err().map(|error| error.to_string());
    }

    pub(crate) fn draw(&mut self, ui: &mut egui::Ui) {
        ui.heading("PlayerConfig");
        ui.monospace(PlayerConfig::PATH);
        ui.separator();
        if let Some(draft) = self.draft.as_mut() {
            if inspector::edit_reflected(ui, draft) {
                self.due = Some(Instant::now() + DEBOUNCE);
                self.error = None;
            }
            ui.separator();
            if let Some(error) = &self.error {
                ui.label(format!("Não foi possível salvar: {error}"));
                if ui.button("Descartar rascunho").clicked() {
                    self.last_seen = None;
                    self.error = None;
                    self.due = None;
                    self.draft = None;
                    self.original = None;
                    self.awaiting_reload = None;
                }
            } else if self.due.is_some() {
                ui.label("Alterações pendentes…");
                if ui.button("Salvar agora").clicked() {
                    self.flush();
                }
            } else if self.awaiting_reload.is_some() {
                ui.label("Salvo. Aguardando o AssetServer…");
            } else {
                ui.label("Salvamento automático · RON");
            }
        } else {
            ui.label("Aguardando um PlayerConfig válido no disco…");
        }
    }
}

pub(crate) fn update(
    mut editor: ResMut<ConfigEditor>,
    server: Res<AssetServer>,
    assets: Res<Assets<PlayerConfig>>,
    mut handle: Local<Option<Handle<PlayerConfig>>>,
) {
    let handle = handle.get_or_insert_with(|| server.load(PlayerConfig::PATH));
    if let Some(asset) = assets.get(handle) {
        editor.synchronize(asset);
    }
    if editor.due.is_some_and(|due| Instant::now() >= due) {
        editor.flush();
    }
}

fn write_if_unchanged(path: &Path, original: &[u8], contents: &[u8]) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("sem pasta de configuração"))?;
    let mut temp = tempfile::NamedTempFile::new_in(dir)?;
    temp.as_file()
        .set_permissions(fs::metadata(path)?.permissions())?;
    temp.write_all(contents)?;
    if fs::read(path)? != original {
        return Err(io::Error::other(
            "o arquivo mudou no disco; descarte o rascunho para reler",
        ));
    }
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::thread;

    use bevy::{asset::io::AssetSourceBuilder, world_serialization::WorldSerializationPlugin};
    use game::{GamePlugin, Health};

    use super::*;

    #[test]
    fn config_save_reloads_through_bevy_and_updates_running_game() {
        let folder = tempfile::tempdir().unwrap();
        let assets = folder.path().join("assets");
        let scenes = folder.path().join("scenes");
        let path = assets.join(PlayerConfig::PATH);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::create_dir(&scenes).unwrap();
        fs::write(
            &path,
            "(velocidade: 150.0, vida_maxima: 100.0, gravidade: 9.8)",
        )
        .unwrap();
        fs::write(
            scenes.join("level.scn.ron"),
            include_str!("../../../scenes/level.scn.ron"),
        )
        .unwrap();
        let mut app = App::new();
        app.register_asset_source(
            "scenes",
            AssetSourceBuilder::platform_default(&scenes.to_string_lossy(), None),
        )
        .add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: assets.to_string_lossy().into_owned(),
                watch_for_changes_override: Some(true),
                ..default()
            },
            WorldSerializationPlugin,
            GamePlugin,
        ))
        .insert_resource(ConfigEditor::new(path.clone()))
        .add_systems(Update, update);
        app.finish();
        app.cleanup();
        let mut query = app.world_mut().query::<&Health>();
        let mut wait_for = |app: &mut App, current, max| {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                app.update();
                let editor = app.world().resource::<ConfigEditor>();
                if editor.due.is_none()
                    && editor.awaiting_reload.is_none()
                    && editor
                        .last_seen
                        .as_ref()
                        .is_some_and(|config| config.vida_maxima == max)
                    && query
                        .single(app.world())
                        .is_ok_and(|health| health.current == current && health.max == max)
                {
                    return;
                }
                assert!(
                    Instant::now() < deadline,
                    "config/game not reloaded: {:?}",
                    editor.error
                );
                thread::sleep(Duration::from_millis(5));
            }
        };
        wait_for(&mut app, 90.0, 100.0);
        {
            let mut editor = app.world_mut().resource_mut::<ConfigEditor>();
            editor.draft.as_mut().unwrap().vida_maxima = 75.0;
            editor.due = Some(Instant::now());
        }
        wait_for(&mut app, 75.0, 75.0);
        let saved: PlayerConfig = ron::de::from_bytes(&fs::read(path).unwrap()).unwrap();
        assert_eq!(saved.vida_maxima, 75.0);
    }

    #[test]
    fn draft_does_not_overwrite_an_external_change() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("config.ron");
        fs::write(&path, b"external edit").unwrap();
        let result = write_if_unchanged(&path, b"old value", b"editor draft");
        assert!(result.is_err());
        assert_eq!(fs::read(path).unwrap(), b"external edit");
    }
}
