use std::{io, marker::PhantomData};

use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
    world_serialization::DynamicWorldRoot,
};
use serde::de::DeserializeOwned;

/// Loads the host's chosen initial scene independently of gameplay plugins.
pub struct StartupScene(pub String);

impl Plugin for StartupScene {
    fn build(&self, app: &mut App) {
        let path = format!("scenes://{}", self.0);
        app.add_systems(
            Startup,
            move |mut commands: Commands, assets: Res<AssetServer>| {
                commands.spawn((
                    DynamicWorldRoot(assets.load(path.clone())),
                    Transform::IDENTITY,
                ));
            },
        );
    }
}

/// Loads a typed RON asset through Bevy's AssetServer.
#[derive(TypePath)]
pub struct RonAssetLoader<A>(PhantomData<fn() -> A>);

impl<A> Default for RonAssetLoader<A> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<A: Asset + DeserializeOwned> AssetLoader for RonAssetLoader<A> {
    type Asset = A;
    type Settings = ();
    type Error = io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _context: &mut LoadContext<'_>,
    ) -> Result<A, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        ron::de::from_bytes(&bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    fn extensions(&self) -> &[&str] {
        &["ron"]
    }
}
