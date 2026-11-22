use bevy_egui::egui::{self, Color32, CornerRadius, FontId, Margin, Stroke, TextStyle, vec2};

pub(crate) const CANVAS: Color32 = Color32::from_rgb(0x18, 0x16, 0x15);
pub(crate) const PANEL: Color32 = Color32::from_rgb(0x29, 0x26, 0x24);
pub(crate) const DEEP: Color32 = Color32::from_rgb(0x1f, 0x1d, 0x1b);
pub(crate) const SURFACE: Color32 = Color32::from_rgb(0x35, 0x32, 0x2f);
pub(crate) const INPUT: Color32 = Color32::from_rgb(0x3d, 0x39, 0x36);
pub(crate) const HOVER: Color32 = Color32::from_rgb(0x45, 0x40, 0x3c);
pub(crate) const BORDER: Color32 = Color32::from_rgb(0x3e, 0x3a, 0x37);
pub(crate) const TEXT: Color32 = Color32::from_rgb(0xeb, 0xe7, 0xe4);
pub(crate) const SECONDARY: Color32 = Color32::from_rgb(0xa6, 0xa0, 0x9b);
pub(crate) const MUTED: Color32 = Color32::from_rgb(0x73, 0x6e, 0x6a);
pub(crate) const RUST: Color32 = Color32::from_rgb(0xd9, 0x6a, 0x38);
pub(crate) const RUST_HOVER: Color32 = Color32::from_rgb(0xe8, 0x7a, 0x48);
pub(crate) const RUST_SOFT: Color32 = Color32::from_rgb(0x8f, 0x44, 0x24);
pub(crate) const WARNING: Color32 = Color32::from_rgb(0xe0, 0xa8, 0x4b);
pub(crate) const ERROR: Color32 = Color32::from_rgb(0xd9, 0x53, 0x4f);

pub(crate) const PANEL_RADIUS: u8 = 12;
pub(crate) const CARD_RADIUS: u8 = 8;
pub(crate) const CONTROL_RADIUS: u8 = 6;
pub(crate) const PILL_RADIUS: u8 = u8::MAX;
pub(crate) const GUTTER: i8 = 8;
pub(crate) const PADDING: i8 = 10;
pub(crate) const HEADER: f32 = 32.0;
pub(crate) const ROW: f32 = 24.0;

pub(crate) fn install(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.style_mut_of(egui::Theme::Dark, |style| {
        style.animation_time = 0.11;
        style.spacing.item_spacing = vec2(6.0, 4.0);
        style.spacing.interact_size = vec2(30.0, ROW);
        style.spacing.button_padding = vec2(8.0, 4.0);
        style.spacing.indent = 14.0;
        style.spacing.icon_width = 14.0;
        style.spacing.window_margin = Margin::same(PADDING);
        style.drag_value_text_style = TextStyle::Monospace;
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(10.0)),
            (TextStyle::Body, FontId::proportional(12.0)),
            (TextStyle::Button, FontId::proportional(12.0)),
            (TextStyle::Heading, FontId::proportional(17.0)),
            (TextStyle::Monospace, FontId::monospace(11.0)),
        ]
        .into();
        let mut visual = egui::Visuals::dark();
        visual.panel_fill = PANEL;
        visual.window_fill = PANEL;
        visual.extreme_bg_color = DEEP;
        visual.faint_bg_color = SURFACE;
        visual.text_edit_bg_color = Some(INPUT);
        visual.weak_text_color = Some(MUTED);
        visual.disabled_alpha = 0.45;
        visual.window_corner_radius = PANEL_RADIUS.into();
        visual.menu_corner_radius = CARD_RADIUS.into();
        visual.window_stroke = Stroke::new(1.0, BORDER);
        visual.selection.bg_fill = SURFACE;
        visual.selection.stroke = Stroke::new(1.5, RUST);

        let w = &mut visual.widgets;
        for (widget, fill, text) in [
            (&mut w.noninteractive, PANEL, SECONDARY),
            (&mut w.inactive, INPUT, TEXT),
            (&mut w.hovered, HOVER, TEXT),
            (&mut w.active, SURFACE, TEXT),
            (&mut w.open, SURFACE, TEXT),
        ] {
            widget.bg_fill = fill;
            widget.weak_bg_fill = fill;
            widget.bg_stroke = Stroke::new(1.0, BORDER);
            widget.fg_stroke = Stroke::new(1.5, text);
            widget.corner_radius = CONTROL_RADIUS.into();
            widget.expansion = 0.0;
        }
        w.active.bg_stroke = Stroke::new(1.5, RUST);
        visual.override_text_color = Some(TEXT);
        style.visuals = visual;
    });
}

pub(crate) fn dock(style: &egui::Style) -> egui_dock::Style {
    let mut dock = egui_dock::Style::from_egui(style);
    dock.separator.width = f32::from(GUTTER);
    dock.separator.extra = 100.0;
    dock.separator.color_idle = CANVAS;
    dock.separator.color_hovered = BORDER;
    dock.separator.color_dragged = HOVER;
    dock.main_surface_border_stroke = Stroke::NONE;
    dock.tab_bar.height = HEADER;
    dock.tab_bar.bg_fill = PANEL;
    dock.tab_bar.hline_color = BORDER;
    dock.tab_bar.corner_radius = CornerRadius {
        nw: PANEL_RADIUS,
        ne: PANEL_RADIUS,
        ..CornerRadius::ZERO
    };
    dock.tab_bar.inner_margin = Margin::symmetric(5, 2);
    dock.tab.tab_body.bg_fill = PANEL;
    dock.tab.tab_body.stroke = Stroke::NONE;
    dock.tab.tab_body.inner_margin = Margin::ZERO;
    dock.tab.tab_body.corner_radius = CornerRadius {
        sw: PANEL_RADIUS,
        se: PANEL_RADIUS,
        ..CornerRadius::ZERO
    };
    for (tab, fill, color) in [
        (&mut dock.tab.active, PANEL, TEXT),
        (&mut dock.tab.focused, PANEL, TEXT),
        (&mut dock.tab.inactive, PANEL, SECONDARY),
        (&mut dock.tab.hovered, HOVER, TEXT),
        (&mut dock.tab.active_with_kb_focus, SURFACE, TEXT),
        (&mut dock.tab.inactive_with_kb_focus, SURFACE, SECONDARY),
        (&mut dock.tab.focused_with_kb_focus, SURFACE, TEXT),
    ] {
        tab.bg_fill = fill;
        tab.text_color = color;
        tab.outline_color = Color32::TRANSPARENT;
        tab.corner_radius = CONTROL_RADIUS.into();
    }
    dock
}

pub(crate) fn label(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).color(SECONDARY)
}

pub(crate) fn section(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).size(12.0).strong().color(TEXT)
}

pub(crate) fn left_label(
    ui: &mut egui::Ui,
    width: f32,
    text: impl Into<egui::WidgetText>,
) -> egui::Response {
    ui.allocate_ui_with_layout(
        vec2(width, ROW),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_width(width);
            ui.add(egui::Label::new(text).truncate())
        },
    )
    .inner
}

pub(crate) fn row(
    ui: &mut egui::Ui,
    selected: bool,
    text: impl Into<egui::WidgetText>,
    icon: crate::icons::Icon,
) -> egui::Response {
    let text = text.into();
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), ROW), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            selected,
            text.text(),
        )
    });
    let fill = if response.hovered() {
        HOVER
    } else if selected {
        SURFACE
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CONTROL_RADIUS, fill);
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            CONTROL_RADIUS,
            Stroke::new(1.5, RUST),
            egui::StrokeKind::Inside,
        );
    }
    let color = if selected || response.hovered() {
        TEXT
    } else {
        SECONDARY
    };
    crate::icons::paint(
        ui.painter(),
        egui::pos2(rect.left() + 12.0, rect.center().y),
        icon,
        color,
    );
    let galley = text.into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        (rect.width() - 30.0).max(1.0),
        TextStyle::Body,
    );
    let pos = egui::pos2(rect.left() + 24.0, rect.center().y - galley.size().y * 0.5);
    ui.painter().galley(pos, galley, color);
    response
}
