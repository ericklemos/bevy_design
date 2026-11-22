use bevy::prelude::*;
use bevy_egui::{
    EguiContext, EguiPreUpdateSet,
    egui::{
        self, Align2, Color32, FontFamily, FontId, Painter, Pos2, RichText, Vec2,
        epaint::text::{FontInsert, FontPriority, InsertFontFamily},
        vec2,
    },
};

use crate::theme;

const FONT_NAME: &str = "Material Symbols Outlined";
const SIZE: f32 = 16.0;

pub(crate) struct MaterialSymbols;

impl Plugin for MaterialSymbols {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            install
                .after(EguiPreUpdateSet::InitContexts)
                .before(EguiPreUpdateSet::BeginPass),
        );
    }
}

fn install(mut contexts: Query<&mut EguiContext, Added<EguiContext>>) {
    for mut context in &mut contexts {
        add_font(context.get_mut());
    }
}

pub(crate) fn add_font(context: &egui::Context) {
    context.add_font(FontInsert::new(
        FONT_NAME,
        egui::FontData::from_static(include_bytes!(
            "../assets/material-symbols/MaterialSymbolsOutlined.ttf"
        )),
        vec![InsertFontFamily {
            family: font_family(),
            priority: FontPriority::Highest,
        }],
    ));
}

fn font_family() -> FontFamily {
    FontFamily::Name(FONT_NAME.into())
}

#[derive(Clone, Copy)]
pub(crate) enum Icon {
    More = 0xe5d4,      // more_vert
    Sort = 0xe164,      // sort
    Filter = 0xef4f,    // filter_alt
    View = 0xe30c,      // desktop_windows
    Play = 0xe037,      // play_arrow
    Pause = 0xe034,     // pause
    Stop = 0xe047,      // stop
    Terminal = 0xeb8e,  // terminal
    Clock = 0xefd6,     // schedule
    Tree = 0xe97a,      // account_tree
    Graph = 0xf38d,     // flowchart
    Inspector = 0xe429, // tune
    Folder = 0xe2c7,    // folder
    Entity = 0xf720,    // deployed_code
    Player = 0xf0d3,    // person
    Eye = 0xe8f4,       // visibility
    EyeOff = 0xe8f5,    // visibility_off
    Lock = 0xe899,      // lock
    LockOpen = 0xe898,  // lock_open
    Collapsed = 0xe5cc, // chevron_right
    Expanded = 0xe5cf,  // expand_more
    Close = 0xe5cd,     // close
}

impl Icon {
    fn glyph(self) -> char {
        char::from_u32(self as u32).expect("Material Symbols codepoint")
    }

    pub(crate) fn text(self) -> RichText {
        RichText::new(self.glyph().to_string()).font(FontId::new(SIZE, font_family()))
    }
}

pub(crate) fn paint(p: &Painter, center: Pos2, icon: Icon, color: Color32) {
    p.text(
        center,
        Align2::CENTER_CENTER,
        icon.glyph(),
        FontId::new(SIZE, font_family()),
        color,
    );
}

pub(crate) fn button(ui: &mut egui::Ui, icon: Icon, active: bool, label: &str) -> egui::Response {
    sized_button(ui, vec2(24.0, theme::ROW), icon, active, label)
}

pub(crate) fn sized_button(
    ui: &mut egui::Ui,
    size: Vec2,
    icon: Icon,
    active: bool,
    label: &str,
) -> egui::Response {
    let response = ui.add_sized(
        size,
        egui::Button::new("")
            .selected(active)
            .frame_when_inactive(active),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), active, label)
    });
    paint(
        ui.painter(),
        response.rect.center(),
        icon,
        if response.hovered() || active {
            theme::TEXT
        } else {
            theme::SECONDARY
        },
    );
    response.on_hover_text(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_symbols_have_glyphs_in_the_first_pass() {
        let context = egui::Context::default();
        add_font(&context);
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            ui.fonts_mut(|fonts| {
                let font = FontId::new(SIZE, font_family());
                for icon in [
                    Icon::More,
                    Icon::Sort,
                    Icon::Filter,
                    Icon::View,
                    Icon::Play,
                    Icon::Pause,
                    Icon::Stop,
                    Icon::Terminal,
                    Icon::Clock,
                    Icon::Tree,
                    Icon::Graph,
                    Icon::Inspector,
                    Icon::Folder,
                    Icon::Entity,
                    Icon::Player,
                    Icon::Eye,
                    Icon::EyeOff,
                    Icon::Lock,
                    Icon::LockOpen,
                    Icon::Collapsed,
                    Icon::Expanded,
                    Icon::Close,
                ] {
                    assert!(
                        fonts.has_glyph(&font, icon.glyph()),
                        "U+{:04X}",
                        icon as u32
                    );
                }
            });
        });
        output.textures_delta.clear();
    }
}
