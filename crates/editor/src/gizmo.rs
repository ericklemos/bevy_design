use bevy::{math::Affine3A, prelude::*};
use bevy_egui::egui::{self, Color32, Pos2, Rect, Vec2 as ScreenVec};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Tool {
    #[default]
    Move,
    Rotate,
    Scale,
}

#[derive(Clone, Copy)]
pub(crate) struct Target {
    pub(crate) entity: Entity,
    pub(crate) local: Transform,
    pub(crate) global: GlobalTransform,
    pub(crate) parent: Option<GlobalTransform>,
}

struct Handle {
    axis: usize,
    a: Pos2,
    b: Pos2,
    units_per_point: f32,
}

struct Drag {
    target: Target,
    tool: Tool,
    axis: usize,
    origin: Pos2,
    direction: ScreenVec,
    units_per_point: f32,
}

#[derive(Default)]
pub(crate) struct Gizmo {
    tool: Tool,
    drag: Option<Drag>,
    snap: bool,
}

const AXES: [Vec3; 3] = [Vec3::X, Vec3::Y, Vec3::Z];
const COLORS: [Color32; 3] = [
    Color32::from_rgb(238, 92, 101),
    Color32::from_rgb(112, 212, 126),
    Color32::from_rgb(91, 162, 255),
];

pub(crate) fn orientation(ui: &egui::Ui, center: Pos2, rotation: Quat) {
    let painter = ui.painter();
    painter.circle_filled(center, 28.0, crate::theme::SURFACE);
    let axes = AXES.map(|axis| rotation.inverse() * axis);
    let mut order = [0, 1, 2];
    order.sort_by(|&a, &b| axes[a].z.total_cmp(&axes[b].z));
    for i in order {
        let axis = axes[i];
        let color = if axis.z > 0.0 {
            COLORS[i]
        } else {
            COLORS[i].gamma_multiply(0.68)
        };
        let point = center + ScreenVec::new(axis.x, -axis.y) * 19.0;
        painter.line_segment([center, point], egui::Stroke::new(1.5, color));
        painter.circle_filled(point, 6.0, color);
        painter.text(
            point,
            egui::Align2::CENTER_CENTER,
            ["X", "Y", "Z"][i],
            egui::FontId::monospace(8.0),
            crate::theme::CANVAS,
        );
    }
}

impl Gizmo {
    pub(crate) fn toolbar(&mut self, ui: &mut egui::Ui, enabled: bool) {
        ui.add_enabled_ui(enabled, |ui| {
            ui.selectable_value(&mut self.tool, Tool::Move, "Mover");
            ui.selectable_value(&mut self.tool, Tool::Rotate, "Girar");
            ui.selectable_value(&mut self.tool, Tool::Scale, "Escalar");
            egui::ComboBox::from_id_salt("snap")
                .width(80.0)
                .selected_text(if self.snap { "Snap on" } else { "Snap off" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.snap, false, "Desligado");
                    ui.selectable_value(&mut self.snap, true, "0.5 u / 15° / 0.1×");
                });
        });
    }

    pub(crate) fn draw(
        &mut self,
        response: &egui::Response,
        camera: &Camera,
        view: Transform,
        target: Option<Target>,
    ) -> Option<(Entity, Transform)> {
        let Some(target) = target else {
            self.drag = None;
            return None;
        };
        let (pointer, down, pressed) = response.ctx.input(|input| {
            (
                input.pointer.interact_pos(),
                input.pointer.primary_down(),
                input.pointer.primary_pressed(),
            )
        });
        if !down
            || self
                .drag
                .as_ref()
                .is_some_and(|drag| drag.target.entity != target.entity || drag.tool != self.tool)
        {
            self.drag = None;
        }
        let view = GlobalTransform::from(view);
        let rect = response.rect;
        let handles = self.handles(camera, &view, rect, target);
        let hovered = pointer.filter(|_| response.hovered()).and_then(|p| {
            handles
                .iter()
                .filter_map(|handle| {
                    let distance = distance_to_segment(p, handle.a, handle.b);
                    (distance < 9.0 && (handle.b - handle.a).length_sq() > 1.0)
                        .then_some((distance, handle))
                })
                .min_by(|(a, _), (b, _)| a.total_cmp(b))
                .map(|(_, handle)| handle)
        });
        if pressed && let (Some(pointer), Some(handle)) = (pointer, hovered) {
            self.drag = Some(Drag {
                target,
                tool: self.tool,
                axis: handle.axis,
                origin: pointer,
                direction: (handle.b - handle.a).normalized(),
                units_per_point: handle.units_per_point,
            });
        }
        let active = self
            .drag
            .as_ref()
            .map(|drag| drag.axis)
            .or_else(|| hovered.map(|handle| handle.axis));
        let painter = response
            .ctx
            .layer_painter(response.layer_id)
            .with_clip_rect(rect);
        for handle in &handles {
            let color = COLORS[handle.axis];
            let width = if active == Some(handle.axis) {
                4.0
            } else {
                2.0
            };
            let stroke = egui::Stroke::new(width, color);
            painter.line_segment([handle.a, handle.b], stroke);
            if self.tool == Tool::Scale {
                painter.rect_filled(
                    Rect::from_center_size(handle.b, ScreenVec::splat(8.0)),
                    1.0,
                    color,
                );
            } else if self.tool == Tool::Move {
                let direction = (handle.b - handle.a).normalized();
                let base = handle.b - direction * 10.0;
                let side = direction.rot90() * 5.0;
                painter.add(egui::Shape::convex_polygon(
                    vec![handle.b, base + side, base - side],
                    color,
                    egui::Stroke::NONE,
                ));
            }
        }
        let (drag, pointer) = (self.drag.as_ref()?, pointer?);
        let delta = (pointer - drag.origin).dot(drag.direction) * drag.units_per_point;
        let delta = if self.snap {
            match drag.tool {
                Tool::Move => (delta / 0.5).round() * 0.5,
                Tool::Rotate => {
                    let step = 15_f32.to_radians();
                    (delta / step).round() * step
                }
                Tool::Scale => ((delta.exp() * 10.0).round().clamp(1.0, 1000.0) * 0.1).ln(),
            }
        } else {
            delta
        };
        Some((drag.target.entity, drag.transform(delta)))
    }

    fn handles(
        &self,
        camera: &Camera,
        view: &GlobalTransform,
        rect: Rect,
        target: Target,
    ) -> Vec<Handle> {
        let pivot = target.global.translation();
        let Some(center) = project(camera, view, rect, pivot) else {
            return Vec::new();
        };
        let determinant = target.global.affine().matrix3.determinant();
        let rotation = if determinant.is_finite() && determinant.abs() > f32::EPSILON {
            target.global.rotation()
        } else {
            target.parent.map_or(Quat::IDENTITY, |parent| {
                let det = parent.affine().matrix3.determinant();
                if det.is_finite() && det.abs() > f32::EPSILON {
                    parent.rotation()
                } else {
                    Quat::IDENTITY
                }
            }) * target.local.rotation
        };
        let radius = (pivot.distance(view.translation()) * 0.14).clamp(0.1, 100.0);
        let mut handles = Vec::new();
        for (i, axis) in AXES.into_iter().enumerate() {
            if self.tool == Tool::Rotate {
                const SEGMENTS: usize = 48;
                let a = rotation * AXES[(i + 1) % 3];
                let b = rotation * AXES[(i + 2) % 3];
                let step = std::f32::consts::TAU / SEGMENTS as f32;
                let mut prev = project(camera, view, rect, pivot + a * radius);
                for s in 1..=SEGMENTS {
                    let (sin, cos) = (step * s as f32).sin_cos();
                    let next = project(camera, view, rect, pivot + (a * cos + b * sin) * radius);
                    if let (Some(a), Some(b)) = (prev, next) {
                        handles.push(Handle {
                            axis: i,
                            a,
                            b,
                            units_per_point: (step / a.distance(b).max(1.0)).min(0.045),
                        });
                    }
                    prev = next;
                }
            } else {
                let world_axis = if self.tool == Tool::Move {
                    axis
                } else {
                    rotation * axis
                };
                let Some(end) = project(camera, view, rect, pivot + world_axis * radius) else {
                    continue;
                };
                let length = center.distance(end);
                if length > 12.0 {
                    let start = center + (end - center).normalized() * 8.0;
                    handles.push(Handle {
                        axis: i,
                        a: start,
                        b: end,
                        units_per_point: if self.tool == Tool::Move {
                            radius / length
                        } else {
                            1.0 / length
                        },
                    });
                }
            }
        }
        handles
    }
}

impl Drag {
    fn transform(&self, delta: f32) -> Transform {
        let mut result = self.target.local;
        let axis = AXES[self.axis];
        match self.tool {
            Tool::Move => {
                let parent_inverse = self
                    .target
                    .parent
                    .map_or(Affine3A::IDENTITY, |parent| parent.affine().inverse());
                let offset = parent_inverse.transform_vector3(axis * delta);
                if offset.is_finite() {
                    result.translation += offset;
                }
            }
            Tool::Rotate => {
                result.rotation = (result.rotation * Quat::from_axis_angle(axis, delta)).normalize()
            }
            Tool::Scale => result.scale[self.axis] *= delta.exp().clamp(0.01, 100.0),
        }
        result
    }
}

fn project(camera: &Camera, view: &GlobalTransform, rect: Rect, point: Vec3) -> Option<Pos2> {
    let ndc = camera.world_to_ndc(view, point)?;
    (ndc.is_finite() && (0.0..=1.0).contains(&ndc.z)).then(|| {
        rect.center() + egui::vec2(ndc.x * rect.width() * 0.5, -ndc.y * rect.height() * 0.5)
    })
}

fn distance_to_segment(point: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let relative = ((point - a).dot(ab) / ab.length_sq().max(f32::EPSILON)).clamp(0.0, 1.0);
    (point - (a + ab * relative)).length()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gizmo_translation_is_world_space_and_rotation_and_scale_are_local() {
        let mut drag = Drag {
            target: Target {
                entity: Entity::PLACEHOLDER,
                local: Transform::IDENTITY,
                global: GlobalTransform::IDENTITY,
                parent: Some(GlobalTransform::from(Transform::from_rotation(
                    Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
                ))),
            },
            tool: Tool::Move,
            axis: 0,
            origin: Pos2::ZERO,
            direction: ScreenVec::X,
            units_per_point: 1.0,
        };
        assert!(
            drag.transform(2.0)
                .translation
                .abs_diff_eq(-2.0 * Vec3::Y, 1e-5)
        );
        drag.tool = Tool::Rotate;
        let rotation = drag.transform(std::f32::consts::FRAC_PI_2).rotation;
        assert!((rotation * Vec3::Y).abs_diff_eq(Vec3::Z, 1e-5));
        drag.tool = Tool::Scale;
        assert!(
            drag.transform(2.0_f32.ln())
                .scale
                .abs_diff_eq(Vec3::new(2.0, 1.0, 1.0), 1e-5)
        );
    }
}
