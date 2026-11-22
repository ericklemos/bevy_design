use std::time::Duration;

use bevy::{
    asset::RenderAssetUsages,
    camera::RenderTarget,
    light::NotShadowCaster,
    prelude::*,
    render::render_resource::{Extent3d, PrimitiveTopology, TextureFormat},
};
use bevy_egui::{
    EguiGlobalSettings, EguiTextureHandle, EguiUserTextures, PrimaryEguiContext, egui,
};
use game::Health;

use crate::{EditorOnly, ui::Workspace};

#[derive(Component)]
pub(crate) struct EditCamera;

#[derive(Resource)]
pub(crate) struct Viewport {
    pub(crate) image: Handle<Image>,
    requested_size: Option<UVec2>,
    pending_resize: Option<(UVec2, Duration)>,
}

const RESIZE_DELAY: Duration = Duration::from_millis(100);

impl Viewport {
    pub(crate) fn request_size(&mut self, size: Option<egui::Vec2>, scale: f32) {
        let pixels = |points: f32| (points * scale).round().clamp(1.0, 4096.0) as u32;
        self.requested_size = size.map(|size| UVec2::new(pixels(size.x), pixels(size.y)));
    }

    fn resize(&mut self, images: &mut Assets<Image>, now: Duration) -> bool {
        let Some(size) = self.requested_size else {
            self.pending_resize = None;
            return false;
        };
        let Some(image) = images.get(&self.image) else {
            return false;
        };
        if image.size() == size {
            self.pending_resize = None;
            return false;
        }
        match self.pending_resize {
            Some((pending, since)) if pending == size => {
                if now.saturating_sub(since) < RESIZE_DELAY {
                    return false;
                }
            }
            _ => {
                self.pending_resize = Some((size, now));
                return false;
            }
        }
        let Some(mut image) = images.get_mut(&self.image) else {
            return false;
        };
        image.resize(Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        });
        self.pending_resize = None;
        true
    }
}

pub(crate) fn resize_target(
    mut viewport: ResMut<Viewport>,
    mut images: ResMut<Assets<Image>>,
    mut projection: Single<&mut Projection, With<EditCamera>>,
    time: Res<Time<Real>>,
) {
    if viewport.resize(&mut images, time.elapsed()) {
        // Bevy emits the image's asset event after the camera update. Refresh
        // the projection now so rendering uses the new size in the same frame.
        projection.set_changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_waits_for_the_panel_size_to_settle() {
        let mut images = Assets::<Image>::default();
        let image = images.add(Image::new_target_texture(
            960,
            540,
            TextureFormat::Bgra8UnormSrgb,
            None,
        ));
        let mut viewport = Viewport {
            image: image.clone(),
            requested_size: None,
            pending_resize: None,
        };

        viewport.request_size(Some(egui::vec2(800.0, 480.0)), 1.25);
        assert!(!viewport.resize(&mut images, Duration::ZERO));
        viewport.request_size(Some(egui::vec2(801.0, 480.0)), 1.25);
        assert!(!viewport.resize(&mut images, Duration::from_millis(90)));
        assert!(!viewport.resize(&mut images, Duration::from_millis(189)));
        assert_eq!(images.get(&image).unwrap().size(), UVec2::new(960, 540));

        assert!(viewport.resize(&mut images, Duration::from_millis(190)));
        assert_eq!(images.get(&image).unwrap().size(), UVec2::new(1001, 600));
        assert!(!viewport.resize(&mut images, Duration::from_millis(300)));

        viewport.request_size(Some(egui::vec2(800.0, 480.0)), 1.25);
        assert!(!viewport.resize(&mut images, Duration::from_millis(310)));
        viewport.request_size(None, 1.25);
        assert!(!viewport.resize(&mut images, Duration::from_millis(400)));
        viewport.request_size(Some(egui::vec2(800.0, 480.0)), 1.25);
        assert!(!viewport.resize(&mut images, Duration::from_millis(500)));
        assert!(viewport.resize(&mut images, Duration::from_millis(600)));
    }
}

#[derive(Resource)]
pub(crate) struct FlyCamera {
    pub(crate) transform: Transform,
    yaw: f32,
    pitch: f32,
}

impl Default for FlyCamera {
    fn default() -> Self {
        let transform = Transform::from_xyz(5.0, 4.0, 7.0).looking_at(Vec3::Y * 0.6, Vec3::Y);
        let (yaw, pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
        Self {
            transform,
            yaw,
            pitch,
        }
    }
}

impl FlyCamera {
    pub(crate) fn navigate(&mut self, response: &egui::Response, dt: f32) {
        if response.dragged_by(egui::PointerButton::Secondary) {
            let drag = response.drag_delta();
            self.yaw -= drag.x * 0.005;
            self.pitch = (self.pitch - drag.y * 0.005).clamp(-1.55, 1.55);
            self.transform.rotation = Quat::from_euler(EulerRot::YXZ, self.yaw, self.pitch, 0.0);
            let movement = response.ctx.input(|i| {
                let axis = |positive, negative| {
                    f32::from(i.key_down(positive)) - f32::from(i.key_down(negative))
                };
                Vec3::new(
                    axis(egui::Key::D, egui::Key::A),
                    axis(egui::Key::E, egui::Key::Q),
                    axis(egui::Key::S, egui::Key::W),
                )
            });
            self.transform.translation +=
                self.transform.rotation * movement.normalize_or_zero() * dt * 7.0;
        }
        if response.hovered() {
            let scroll = response.ctx.input(|i| i.smooth_scroll_delta.y);
            self.transform.translation += self.transform.forward() * scroll * 0.015;
        }
    }
}

#[derive(Component)]
pub(crate) struct PreviewCube;

#[derive(Component)]
pub(crate) struct PreviewGrid;

fn grid_mesh() -> Mesh {
    // Thin quads keep the grid in world space and correctly occluded by the preview mesh.
    let mut positions = Vec::<[f32; 3]>::new();
    let extent = 20.0;
    for i in -19_i32..=19 {
        let axis = i as f32;
        let half_width = if i % 5 == 0 { 0.018 } else { 0.009 };
        let (a, b) = (axis - half_width, axis + half_width);
        for rotated in [false, true] {
            let corners = if rotated {
                [[-extent, a], [-extent, b], [extent, a], [extent, b]]
            } else {
                [[a, -extent], [a, extent], [b, -extent], [b, extent]]
            };
            for index in [0, 1, 2, 2, 1, 3] {
                let [x, z] = corners[index];
                positions.push([x, 0.004, z]);
            }
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        vec![[0.0, 1.0, 0.0]; positions.len()],
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
}

pub(crate) fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut textures: ResMut<EguiUserTextures>,
    mut settings: ResMut<EguiGlobalSettings>,
) {
    settings.auto_create_primary_context = false;
    let image = images.add(Image::new_target_texture(
        960,
        540,
        TextureFormat::Bgra8UnormSrgb,
        None,
    ));
    textures.add_image(EguiTextureHandle::Strong(image.clone()));

    let camera = FlyCamera::default();
    commands.spawn((Camera2d, PrimaryEguiContext, EditorOnly));
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.15, 0.14, 0.13)),
            ..default()
        },
        RenderTarget::Image(image.clone().into()),
        camera.transform,
        EditCamera,
        EditorOnly,
    ));
    commands.insert_resource(Viewport {
        image,
        requested_size: None,
        pending_resize: None,
    });
    commands.insert_resource(camera);

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.5, 1.5, 1.5))),
        MeshMaterial3d(materials.add(Color::srgb(0.25, 0.55, 0.78))),
        Transform::from_xyz(0.0, 0.75, 0.0),
        Visibility::Hidden,
        PreviewCube,
        EditorOnly,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(40.0, 40.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.13, 0.12, 0.11))),
        EditorOnly,
    ));
    commands.spawn((
        Mesh3d(meshes.add(grid_mesh())),
        MeshMaterial3d(materials.add(Color::srgb(0.19, 0.175, 0.16))),
        Transform::IDENTITY,
        PreviewGrid,
        NotShadowCaster,
        EditorOnly,
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(3.0, 8.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        EditorOnly,
    ));
}

pub(crate) fn update_preview(
    health: Query<(Entity, &Health, &GlobalTransform), Without<PreviewCube>>,
    workspace: Res<Workspace>,
    mut preview: Single<
        (
            &mut Visibility,
            &mut GlobalTransform,
            &MeshMaterial3d<StandardMaterial>,
        ),
        With<PreviewCube>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut grid: Single<&mut Visibility, (With<PreviewGrid>, Without<PreviewCube>)>,
) {
    let grid_visibility = if workspace.grid_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    grid.set_if_neq(grid_visibility);
    let (visibility, transform, material) = &mut *preview;
    if let Ok((entity, health, global)) = health.single()
        && workspace.preview_visible(entity)
    {
        **visibility = Visibility::Visible;
        **transform = *global;
        let health_fraction = if health.max > 0.0 {
            (health.current / health.max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let color = Color::srgb(
            0.64 - 0.39 * health_fraction,
            0.27 + 0.28 * health_fraction,
            0.36 + 0.42 * health_fraction,
        );
        if materials
            .get(&material.0)
            .is_some_and(|value| value.base_color != color)
            && let Some(mut value) = materials.get_mut(&material.0)
        {
            value.base_color = color;
        }
    } else {
        **visibility = Visibility::Hidden;
    }
}
