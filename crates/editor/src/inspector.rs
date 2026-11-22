use std::any::TypeId;

use bevy::{
    prelude::*,
    reflect::{PartialReflect, ReflectMut, ReflectRef, std_traits::ReflectDefault},
    transform::components::TransformTreeChanged,
};
use bevy_egui::egui;

use crate::{
    EditorOnly,
    icons::{self, Icon},
    theme,
    ui::Workspace,
};

#[derive(Clone)]
pub(crate) enum Step {
    Struct(usize),
    Tuple(usize),
    TupleStruct(usize),
    List(usize),
    Array(usize),
}

macro_rules! values {
    ($( $variant:ident : $ty:ty ),* $(,)?) => {
        #[derive(Clone)]
        pub(crate) enum Value {
            $($variant($ty),)*
            Bool(bool),
            Text(String),
            Rotation(Quat),
            Vector(Vec3),
            Color(Color),
            ReadOnly(String),
        }

        impl Value {
            fn read(value: &dyn PartialReflect) -> Self {
                $(if let Some(value) = value.try_downcast_ref::<$ty>() {
                    return Self::$variant(*value);
                })*
                if let Some(value) = value.try_downcast_ref::<bool>() {
                    Self::Bool(*value)
                } else if let Some(value) = value.try_downcast_ref::<String>() {
                    Self::Text(value.clone())
                } else if let Some(value) = value.try_downcast_ref::<Quat>() {
                    Self::Rotation(*value)
                } else if let Some(value) = value.try_downcast_ref::<Vec3>() {
                    Self::Vector(*value)
                } else if let Some(value) = value.try_downcast_ref::<Color>() {
                    Self::Color(*value)
                } else {
                    Self::ReadOnly(format!("{value:?}"))
                }
            }

            fn edit(&mut self, ui: &mut egui::Ui) -> bool {
                match self {
                    $(Self::$variant(value) => {
                        let float = stringify!($ty).starts_with('f');
                        let drag = egui::DragValue::new(value).speed(if float { 0.05 } else { 1.0 });
                        ui.add_sized([ui.available_width(), theme::ROW], drag).changed()
                    },)*
                    Self::Bool(value) => ui.checkbox(value, "").changed(),
                    Self::Text(value) => ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY)).changed(),
                    Self::Vector(value) => {
                        let mut xyz = value.to_array();
                        if vector_fields(ui, &mut xyz, 0.05, 3, "") {
                            *value = Vec3::from_array(xyz);
                            true
                        } else {
                            false
                        }
                    }
                    Self::Rotation(value) => {
                        let (x, y, z) = value.to_euler(EulerRot::XYZ);
                        let mut degrees = [x.to_degrees(), y.to_degrees(), z.to_degrees()];
                        let changed = vector_fields(ui, &mut degrees, 0.5, 1, "°");
                        if changed {
                            let [x, y, z] = degrees.map(f32::to_radians);
                            *value = Quat::from_euler(EulerRot::XYZ, x, y, z);
                        }
                        changed
                    }
                    Self::Color(value) => {
                        let color = value.to_srgba();
                        let mut channels = [color.red, color.green, color.blue, color.alpha];
                        if ui.color_edit_button_rgba_unmultiplied(&mut channels).changed() {
                            let [r, g, b, a] = channels;
                            *value = Color::srgba(r, g, b, a);
                            true
                        } else {
                            false
                        }
                    }
                    Self::ReadOnly(value) => {
                        ui.add(egui::Label::new(theme::label(value.as_str()).monospace()).truncate()).on_hover_text(value.as_str());
                        false
                    }
                }
            }

            fn apply(self, field: &mut dyn PartialReflect) {
                match self {
                    $(Self::$variant(value) => { let _ = field.try_apply(&value); },)*
                    Self::Bool(value) => { let _ = field.try_apply(&value); }
                    Self::Text(value) => { let _ = field.try_apply(&value); }
                    Self::Rotation(value) => { let _ = field.try_apply(&value); }
                    Self::Vector(value) => { let _ = field.try_apply(&value); }
                    Self::Color(value) => { let _ = field.try_apply(&value); }
                    Self::ReadOnly(_) => {}
                }
            }
        }
    };
}

values!(F32: f32, F64: f64, I8: i8, I16: i16, I32: i32, I64: i64, Isize: isize,
        U8: u8, U16: u16, U32: u32, U64: u64, Usize: usize);

fn vector_fields(
    ui: &mut egui::Ui,
    values: &mut [f32; 3],
    speed: f64,
    decimals: usize,
    suffix: &str,
) -> bool {
    let mut changed = false;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.spacing_mut().button_padding.x = 2.0;
        ui.spacing_mut().interact_size.x = 0.0;
        let width = ((ui.available_width() - 8.0) / 3.0).max(18.0);
        for (axis, value) in ["X", "Y", "Z"].into_iter().zip(values) {
            // Keep the axis above the number: even negative values fit in dense docks.
            ui.allocate_ui_with_layout(
                egui::vec2(width, theme::ROW),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(theme::label(axis).small());
                    changed |= ui
                        .add_sized(
                            [width, theme::ROW],
                            egui::DragValue::new(value)
                                .speed(speed)
                                .fixed_decimals(decimals)
                                .suffix(suffix),
                        )
                        .on_hover_text(axis)
                        .changed();
                },
            );
        }
    });
    changed
}

fn property(ui: &mut egui::Ui, label: &str, controls: impl FnOnce(&mut egui::Ui) -> bool) -> bool {
    ui.horizontal(|ui| {
        let width = (ui.available_width() * 0.38).max(16.0);
        theme::left_label(ui, width, theme::label(label)).on_hover_text(label);
        controls(ui)
    })
    .inner
}

struct Field {
    label: String,
    path: Vec<Step>,
    value: Value,
}

fn fields(value: &dyn PartialReflect) -> Vec<Field> {
    const MAX_FIELDS: usize = 128;
    fn collect(value: &dyn PartialReflect, output: &mut Vec<Field>, label: &str, path: &[Step]) {
        if output.len() >= MAX_FIELDS {
            return;
        }
        let mut add = |value, step, name: &str| {
            let mut path = path.to_vec();
            path.push(step);
            let label = if label.is_empty() {
                name.to_owned()
            } else {
                format!("{label}.{name}")
            };
            collect(value, output, &label, &path);
        };
        if path.len() < 8
            && value.try_downcast_ref::<Quat>().is_none()
            && value.try_downcast_ref::<Vec3>().is_none()
            && value.try_downcast_ref::<Color>().is_none()
        {
            match value.reflect_ref() {
                ReflectRef::Struct(v) if v.field_len() > 0 => {
                    for i in 0..v.field_len().min(MAX_FIELDS) {
                        if let Some(field) = v.field_at(i) {
                            add(field, Step::Struct(i), v.name_at(i).unwrap_or("?"));
                        }
                    }
                    return;
                }
                ReflectRef::Tuple(v) if v.field_len() > 0 => {
                    for (i, field) in v.iter_fields().take(MAX_FIELDS).enumerate() {
                        add(field, Step::Tuple(i), &i.to_string());
                    }
                    return;
                }
                ReflectRef::TupleStruct(v) if v.field_len() > 0 => {
                    for (i, field) in v.iter_fields().take(MAX_FIELDS).enumerate() {
                        add(field, Step::TupleStruct(i), &i.to_string());
                    }
                    return;
                }
                ReflectRef::List(v) if !v.is_empty() => {
                    for (i, field) in v.iter().take(MAX_FIELDS).enumerate() {
                        add(field, Step::List(i), &i.to_string());
                    }
                    return;
                }
                ReflectRef::Array(v) if !v.is_empty() => {
                    for (i, field) in v.iter().take(MAX_FIELDS).enumerate() {
                        add(field, Step::Array(i), &i.to_string());
                    }
                    return;
                }
                _ => {}
            }
        }
        output.push(Field {
            label: if label.is_empty() {
                "Valor".into()
            } else {
                label.into()
            },
            path: path.to_vec(),
            value: Value::read(value),
        });
    }

    let mut result = Vec::new();
    collect(value, &mut result, "", &[]);
    result
}

fn write_leaf(target: &mut dyn PartialReflect, path: &[Step], value: Value) {
    let Some((step, remaining)) = path.split_first() else {
        value.apply(target);
        return;
    };
    let field = match (step, target.reflect_mut()) {
        (&Step::Struct(i), ReflectMut::Struct(v)) => v.field_at_mut(i),
        (&Step::Tuple(i), ReflectMut::Tuple(v)) => v.field_mut(i),
        (&Step::TupleStruct(i), ReflectMut::TupleStruct(v)) => v.field_mut(i),
        (&Step::List(i), ReflectMut::List(v)) => v.get_mut(i),
        (&Step::Array(i), ReflectMut::Array(v)) => v.get_mut(i),
        _ => None,
    };
    if let Some(field) = field {
        write_leaf(field, remaining, value);
    }
}

pub(crate) fn edit_reflected(ui: &mut egui::Ui, value: &mut dyn PartialReflect) -> bool {
    let mut changed = false;
    for (index, mut field) in fields(value).into_iter().enumerate() {
        ui.push_id(index, |ui| {
            if property(ui, &field.label, |ui| field.value.edit(ui)) {
                write_leaf(value, &field.path, field.value);
                changed = true;
            }
        });
    }
    changed
}

struct Section {
    id: TypeId,
    label: String,
    mutable: bool,
    fields: Vec<Field>,
}

#[derive(Default, Resource)]
pub(crate) struct Inspector {
    entity: Option<Entity>,
    sections: Vec<Section>,
    addable: Vec<(String, TypeId)>,
}

fn managed(ty: TypeId) -> bool {
    [
        TypeId::of::<Children>(),
        TypeId::of::<ChildOf>(),
        TypeId::of::<GlobalTransform>(),
        TypeId::of::<TransformTreeChanged>(),
        TypeId::of::<EditorOnly>(),
    ]
    .contains(&ty)
}

pub(crate) fn refresh(world: &mut World) {
    let selected = world.resource::<Workspace>().selected;
    let result = selected.and_then(|id| {
        let entity = world.get_entity(id).ok()?;
        if entity.contains::<EditorOnly>() {
            return None;
        }
        let registry = world.resource::<AppTypeRegistry>().read();
        let mut sections = Vec::new();
        for &component_id in entity.archetype().components() {
            let Some(info) = world.components().get_info(component_id) else {
                continue;
            };
            let Some(ty) = info.type_id().filter(|ty| !managed(*ty)) else {
                continue;
            };
            let Some(registration) = registry.get(ty) else {
                continue;
            };
            let Some(component) = registration
                .data::<ReflectComponent>()
                .and_then(|reflect| reflect.reflect(entity))
            else {
                continue;
            };
            let label = registration
                .type_info()
                .type_path_table()
                .short_path()
                .to_owned();
            let mutable = info.mutable() && ty != TypeId::of::<Name>();
            let fields = if ty == TypeId::of::<Name>() {
                vec![Field {
                    path: Vec::new(),
                    label: "Nome".into(),
                    value: Value::ReadOnly(entity.get::<Name>()?.as_str().into()),
                }]
            } else {
                fields(component.as_partial_reflect())
            };
            sections.push(Section {
                id: ty,
                label,
                mutable,
                fields,
            });
        }
        sections.sort_by(|a, b| {
            (a.id != TypeId::of::<Transform>())
                .cmp(&(b.id != TypeId::of::<Transform>()))
                .then(a.label.cmp(&b.label))
        });
        let mut addable: Vec<_> = registry
            .iter()
            .filter_map(|entry| {
                let ty = entry.type_id();
                let component = entry.data::<ReflectComponent>()?;
                entry.data::<ReflectDefault>()?;
                let path = entry.type_info().type_path_table();
                let supported_builtin =
                    ty == TypeId::of::<Transform>() || ty == TypeId::of::<Name>();
                (!managed(ty)
                    && !component.contains(entity)
                    && (supported_builtin || !path.path().starts_with("bevy_")))
                .then(|| (path.short_path().to_owned(), ty))
            })
            .collect();
        addable.sort_by(|a, b| a.0.cmp(&b.0));
        Some(Inspector {
            entity: Some(id),
            sections,
            addable,
        })
    });
    *world.resource_mut::<Inspector>() = result.unwrap_or_default();
}

pub(crate) enum Edit {
    Set {
        entity: Entity,
        component: TypeId,
        path: Vec<Step>,
        value: Value,
    },
    Add(Entity, TypeId),
    Remove(Entity, TypeId),
}

impl Inspector {
    pub(crate) fn draw(
        &self,
        ui: &mut egui::Ui,
        entity: Entity,
        editable: bool,
        edits: &mut Vec<Edit>,
    ) {
        if self.entity != Some(entity) {
            return;
        }
        for section in &self.sections {
            ui.push_id((entity, section.id), |ui| {
                egui::Frame::new()
                    .fill(theme::DEEP)
                    .stroke(egui::Stroke::new(1.0, theme::BORDER))
                    .corner_radius(theme::CARD_RADIUS)
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        let id = ui.make_persistent_id("component");
                        egui::collapsing_header::CollapsingState::load_with_default_open(
                            ui.ctx(),
                            id,
                            true,
                        )
                        .show_header(ui, |ui| {
                            let title =
                                ui.add(egui::Label::new(theme::section(&section.label)).truncate());
                            title.on_hover_text(&section.label);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let more =
                                        icons::button(ui, Icon::More, false, "Ações do componente");
                                    egui::Popup::menu(&more).show(|ui| {
                                        ui.monospace(&section.label);
                                        if ui
                                            .add_enabled(
                                                editable,
                                                egui::Button::new("Remover componente"),
                                            )
                                            .clicked()
                                        {
                                            edits.push(Edit::Remove(entity, section.id));
                                            ui.close();
                                        }
                                    });
                                    ui.label(theme::label("?"))
                                        .on_hover_text(if section.mutable {
                                            "Componente refletido. Edite os campos no modo Edit."
                                        } else {
                                            "Campos somente para leitura."
                                        });
                                },
                            );
                        })
                        .body_unindented(|ui| {
                            for (index, field) in section.fields.iter().enumerate() {
                                ui.push_id(index, |ui| {
                                    let mut value = field.value.clone();
                                    if property(ui, &field.label, |ui| {
                                        ui.add_enabled_ui(editable && section.mutable, |ui| {
                                            value.edit(ui)
                                        })
                                        .inner
                                    }) {
                                        edits.push(Edit::Set {
                                            entity,
                                            component: section.id,
                                            path: field.path.clone(),
                                            value,
                                        });
                                    }
                                });
                            }
                        });
                    });
            });
        }
        ui.add_space(6.0);
        let trigger = ui.add_enabled(
            editable,
            egui::Button::new("+ Add Component")
                .min_size(egui::vec2(ui.available_width(), theme::ROW)),
        );
        egui::Popup::menu(&trigger).show(|ui| {
            for (label, ty) in &self.addable {
                if ui.button(label).clicked() {
                    edits.push(Edit::Add(entity, *ty));
                    ui.close();
                }
            }
            if self.addable.is_empty() {
                ui.label("Nenhum tipo refletido com Default disponível.");
            }
        });
    }
}

impl Edit {
    pub(crate) fn apply(self, world: &mut World) {
        let (entity, ty) = match &self {
            &Self::Set {
                entity, component, ..
            }
            | &Self::Add(entity, component)
            | &Self::Remove(entity, component) => (entity, component),
        };
        if managed(ty) {
            return;
        }
        let registry = world.resource::<AppTypeRegistry>().clone();
        let registry = registry.read();
        let Some(registration) = registry.get(ty) else {
            return;
        };
        let Some(component) = registration.data::<ReflectComponent>() else {
            return;
        };
        let mutable = world
            .components()
            .get_id(ty)
            .and_then(|id| world.components().get_info(id))
            .is_some_and(|info| info.mutable());
        let Ok(mut target) = world.get_entity_mut(entity) else {
            return;
        };
        if target.contains::<EditorOnly>() {
            return;
        }
        match self {
            Self::Set { path, value, .. } if mutable && ty != TypeId::of::<Name>() => {
                if let Some(mut target) = component.reflect_mut(&mut target) {
                    write_leaf(target.as_partial_reflect_mut(), &path, value);
                }
            }
            Self::Add(..) if !component.contains(&target) => {
                if let Some(default) = registration.data::<ReflectDefault>() {
                    component.insert(
                        &mut target,
                        default.default().as_partial_reflect(),
                        &registry,
                    );
                }
            }
            Self::Remove(..) => component.remove(&mut target),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use game::Health;

    use super::*;

    #[test]
    fn registry_can_add_edit_and_remove_components() {
        let mut app = App::new();
        app.register_type::<Health>().register_type::<Transform>();
        let world = app.world_mut();
        let entity = world.spawn_empty().id();
        Edit::Add(entity, TypeId::of::<Health>()).apply(world);
        Edit::Set {
            entity,
            component: TypeId::of::<Health>(),
            path: vec![Step::Struct(0)],
            value: Value::F32(42.0),
        }
        .apply(world);
        assert_eq!(world.get::<Health>(entity).unwrap().current, 42.0);
        Edit::Remove(entity, TypeId::of::<Health>()).apply(world);
        assert!(world.get::<Health>(entity).is_none());

        Edit::Add(entity, TypeId::of::<Transform>()).apply(world);
        let translation = fields(world.get::<Transform>(entity).unwrap())
            .into_iter()
            .find(|field| field.label == "translation")
            .unwrap();
        assert!(matches!(translation.value, Value::Vector(Vec3::ZERO)));
        Edit::Set {
            entity,
            component: TypeId::of::<Transform>(),
            path: translation.path,
            value: Value::Vector(Vec3::new(5.0, -2.5, 1.0)),
        }
        .apply(world);
        assert_eq!(
            world.get::<Transform>(entity).unwrap().translation,
            Vec3::new(5.0, -2.5, 1.0)
        );
    }
}
