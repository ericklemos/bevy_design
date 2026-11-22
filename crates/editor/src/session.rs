use bevy::{
    ecs::entity::EntityHashMap,
    prelude::*,
    world_serialization::{DynamicWorld, DynamicWorldBuilder},
};
use game::GameSimulation;

use crate::EditorOnly;

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) enum Mode {
    #[default]
    Edit,
    Play,
    Pause,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Action {
    Play,
    Pause,
    Stop,
}

struct Snapshot {
    scene: DynamicWorld,
    // Rebuild relationships with normal ECS hooks. DynamicWorld skips relationship hooks.
    links: Vec<(Entity, Entity)>,
}

#[derive(Default, Resource)]
pub(crate) struct Session {
    pub(crate) mode: Mode,
    pub(crate) restored_root: Option<Entity>,
    snapshot: Option<Snapshot>,
}

pub(crate) fn transition(world: &mut World, root: Entity, action: Action) {
    world.resource_scope(|world, mut session: Mut<Session>| {
        let result = match (&session.mode, action) {
            (Mode::Edit, Action::Play) => {
                session.snapshot = Some(capture(world, root));
                session.mode = Mode::Play;
                Ok("Simulação iniciada.")
            }
            (Mode::Pause, Action::Play) => {
                session.mode = Mode::Play;
                Ok("Simulação retomada.")
            }
            (Mode::Play, Action::Pause) => {
                session.mode = Mode::Pause;
                Ok("Simulação pausada.")
            }
            (Mode::Play | Mode::Pause, Action::Stop) => {
                session.mode = Mode::Pause;
                restore(
                    world,
                    root,
                    session.snapshot.as_ref().expect("active snapshot"),
                )
                .map(|new_root| {
                    session.restored_root = Some(new_root);
                    session.snapshot = None;
                    session.mode = Mode::Edit;
                    "Snapshot restaurado."
                })
            }
            _ => return,
        };
        world.resource_mut::<GameSimulation>().0 = session.mode == Mode::Play;
        match result {
            Ok(status) => {
                info!(target: crate::console::SESSION_TARGET, ?action, mode = ?session.mode, "{status}");
            }
            Err(error) => {
                error!(target: crate::console::SESSION_TARGET, ?action, mode = ?session.mode, "{error}");
            }
        }
    });
}

fn capture(world: &World, root: Entity) -> Snapshot {
    let mut links = Vec::new();
    let mut pending = vec![root];
    while let Some(parent) = pending.pop() {
        let Some(children) = world.get::<Children>(parent) else {
            continue;
        };
        for child in children
            .iter()
            .filter(|child| world.get::<EditorOnly>(*child).is_none())
        {
            links.push((child, parent));
            pending.push(child);
        }
    }
    let registry = world.resource::<AppTypeRegistry>().read();
    let scene = DynamicWorldBuilder::from_world(world, &registry)
        .deny_all_resources()
        .deny_component::<Children>()
        .deny_component::<ChildOf>()
        .deny_component::<GlobalTransform>()
        .extract_entities(links.iter().map(|&(child, _)| child))
        .build();
    Snapshot { scene, links }
}

fn restore(world: &mut World, old_root: Entity, snapshot: &Snapshot) -> Result<Entity, String> {
    let new_root = world.spawn((EditorOnly, Transform::IDENTITY)).id();
    let mut map = EntityHashMap::default();
    map.insert(old_root, new_root);
    if let Err(error) = snapshot.scene.write_to_world(world, &mut map) {
        for &entity in map.values() {
            world.despawn(entity);
        }
        return Err(error.to_string());
    }
    for &(child, parent) in &snapshot.links {
        world.entity_mut(map[&child]).insert(ChildOf(map[&parent]));
    }
    // Preserve editor helpers even if they were temporarily parented under a scene entity.
    let mut pending = vec![old_root];
    while let Some(parent) = pending.pop() {
        let children: Vec<_> = world
            .get::<Children>(parent)
            .map(|children| children.iter().collect())
            .unwrap_or_default();
        for child in children {
            if world.get::<EditorOnly>(child).is_some() {
                world.entity_mut(child).remove::<ChildOf>();
            } else {
                pending.push(child);
            }
        }
    }
    world.despawn(old_root);
    Ok(new_root)
}

#[cfg(test)]
mod tests {
    use std::{
        path::Path,
        thread,
        time::{Duration, Instant},
    };

    use bevy::{
        asset::io::AssetSourceBuilder,
        world_serialization::{
            DynamicWorldRoot, WorldInstance, WorldInstanceSpawner, WorldSerializationPlugin,
        },
    };
    use engine_runtime::StartupScene;
    use game::{GamePlugin, Health};

    use super::*;

    #[test]
    fn new_project_loads_an_empty_scene_and_can_play_and_stop() {
        let parent = tempfile::tempdir().unwrap();
        let project = projects::Project::create(&parent.path().join("vazio"), "Vazio").unwrap();
        let mut app = App::new();
        app.register_asset_source(
            "scenes",
            AssetSourceBuilder::platform_default(&project.scenes().to_string_lossy(), None),
        )
        .add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: project.assets().to_string_lossy().into_owned(),
                ..default()
            },
            WorldSerializationPlugin,
            StartupScene(project.startup_scene().into()),
        ))
        .init_resource::<Session>()
        .insert_resource(GameSimulation(false));
        app.finish();
        app.cleanup();

        let mut roots = app
            .world_mut()
            .query_filtered::<(Entity, &WorldInstance), With<DynamicWorldRoot>>();
        let deadline = Instant::now() + Duration::from_secs(5);
        let root = loop {
            app.update();
            let world = app.world();
            if let Ok((root, instance)) = roots.single(world)
                && world
                    .resource::<WorldInstanceSpawner>()
                    .instance_is_ready(**instance)
            {
                break root;
            }
            assert!(Instant::now() < deadline, "starter scene did not load");
            thread::sleep(Duration::from_millis(1));
        };
        let world = app.world_mut();
        assert!(world.get::<Children>(root).is_none());
        transition(world, root, Action::Play);
        assert_eq!(world.resource::<Session>().mode, Mode::Play);
        transition(world, root, Action::Stop);
        let session = world.resource::<Session>();
        assert_eq!(session.mode, Mode::Edit);
        assert!(
            world
                .get::<Children>(session.restored_root.unwrap())
                .is_none()
        );
    }

    #[test]
    fn stop_restores_reflected_state_and_hierarchy_after_pause_and_resume() {
        let mut app = App::new();
        app.register_type::<Health>()
            .init_resource::<Session>()
            .insert_resource(GameSimulation(false));
        let world = app.world_mut();
        let root = world.spawn_empty().id();
        let player = world
            .spawn((
                Health {
                    current: 120.0,
                    max: 120.0,
                },
                ChildOf(root),
            ))
            .id();
        let child = world.spawn(ChildOf(player)).id();
        let helper = world.spawn((EditorOnly, ChildOf(player))).id();

        transition(world, root, Action::Play);
        assert!(world.resource::<GameSimulation>().0);
        world.get_mut::<Health>(player).unwrap().current = 90.0;
        world.despawn(child);
        let temporary = world.spawn(ChildOf(player)).id();
        transition(world, root, Action::Pause);
        assert!(!world.resource::<GameSimulation>().0);
        transition(world, root, Action::Play);
        assert!(world.resource::<GameSimulation>().0);
        transition(world, root, Action::Stop);

        let session = world.resource::<Session>();
        assert_eq!(session.mode, Mode::Edit);
        assert!(!world.resource::<GameSimulation>().0);
        assert!(world.get_entity(player).is_err());
        assert!(world.get_entity(temporary).is_err());
        assert!(world.get_entity(helper).is_ok());
        let new_root = session.restored_root.unwrap();
        let new_player = world.get::<Children>(new_root).unwrap()[0];
        assert_eq!(world.get::<Children>(new_player).unwrap().len(), 1);
        assert_eq!(world.get::<Health>(new_player).unwrap().current, 120.0);
    }

    #[test]
    fn loaded_demo_can_play_and_restore_twice() {
        let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/demo");
        let mut app = App::new();
        app.register_asset_source(
            "scenes",
            AssetSourceBuilder::platform_default(&project.join("scenes").to_string_lossy(), None),
        )
        .add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: project.join("assets").to_string_lossy().into_owned(),
                watch_for_changes_override: Some(false),
                ..default()
            },
            WorldSerializationPlugin,
            StartupScene("level.scn.ron".into()),
        ))
        .insert_resource(GameSimulation(false))
        .init_resource::<Session>()
        .add_plugins(GamePlugin);
        app.finish();
        app.cleanup();

        let mut players = app.world_mut().query::<&Health>();
        let mut roots = app
            .world_mut()
            .query_filtered::<Entity, With<DynamicWorldRoot>>();
        let mut wait_for = |app: &mut App, expected: (f32, f32)| {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                app.update();
                if players
                    .single(app.world())
                    .is_ok_and(|health| (health.current, health.max) == expected)
                {
                    return;
                }
                assert!(Instant::now() < deadline, "expected Health {expected:?}");
                thread::sleep(Duration::from_millis(1));
            }
        };
        wait_for(&mut app, (120.0, 120.0));
        let mut root = roots.single(app.world()).unwrap();
        for _ in 0..2 {
            transition(app.world_mut(), root, Action::Play);
            wait_for(&mut app, (90.0, 100.0));
            transition(app.world_mut(), root, Action::Stop);
            wait_for(&mut app, (120.0, 120.0));
            root = app.world().resource::<Session>().restored_root.unwrap();
        }
    }
}
