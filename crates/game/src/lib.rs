use bevy::prelude::*;
use engine_runtime::RonAssetLoader;
use serde::{Deserialize, Serialize};

pub struct GamePlugin;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
enum PlayerSystems {
    Gameplay,
}

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Health>()
            .register_type::<Transform>()
            .register_type::<PlayerConfig>()
            .init_asset::<PlayerConfig>()
            .init_asset_loader::<RonAssetLoader<PlayerConfig>>()
            .init_resource::<GameSimulation>()
            .add_observer(apply_damage)
            .add_systems(Startup, load_player_config)
            .add_systems(
                Update,
                (start_demo, refresh_player_limits)
                    .chain()
                    .in_set(PlayerSystems::Gameplay)
                    .run_if(simulation_is_running),
            );
    }
}

/// The game runs by default. Hosts can pause gameplay without changing the Bevy app loop.
#[derive(Resource)]
pub struct GameSimulation(pub bool);

impl Default for GameSimulation {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Component, Debug, Reflect)]
#[reflect(Component, Default)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            current: 120.0,
            max: 120.0,
        }
    }
}

#[derive(Asset, Reflect, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerConfig {
    pub velocidade: f32,
    pub vida_maxima: f32,
    pub gravidade: f32,
}

impl PlayerConfig {
    pub const PATH: &str = "configs/player_config.ron";
}

#[derive(Resource)]
struct ConfigHandle(Handle<PlayerConfig>);

#[derive(Component)]
struct DemoStarted;

#[derive(EntityEvent)]
struct Damage {
    entity: Entity,
    amount: f32,
}

fn load_player_config(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(ConfigHandle(assets.load(PlayerConfig::PATH)));
}

fn simulation_is_running(simulation: Res<GameSimulation>) -> bool {
    simulation.0
}

fn start_demo(
    mut commands: Commands,
    config_handle: Res<ConfigHandle>,
    configs: Res<Assets<PlayerConfig>>,
    mut players: Query<(Entity, &mut Health), Without<DemoStarted>>,
) {
    let Some(config) = configs.get(&config_handle.0) else {
        return;
    };

    for (entity, mut health) in &mut players {
        health.max = config.vida_maxima;
        health.current = health.current.min(health.max);
        commands.entity(entity).insert(DemoStarted);
        commands.trigger(Damage {
            entity,
            amount: 10.0,
        });
    }
}

fn apply_damage(damage: On<Damage>, mut players: Query<&mut Health>) {
    if let Ok(mut health) = players.get_mut(damage.entity) {
        health.current = (health.current - damage.amount).max(0.0);
    }
}

fn refresh_player_limits(
    config_handle: Res<ConfigHandle>,
    configs: Res<Assets<PlayerConfig>>,
    mut players: Query<&mut Health, With<DemoStarted>>,
) {
    let Some(config) = configs.get(&config_handle.0) else {
        return;
    };
    let max = config.vida_maxima.max(0.0);
    for mut health in &mut players {
        if health.max != max {
            health.max = max;
            health.current = health.current.min(max);
        }
    }
}
