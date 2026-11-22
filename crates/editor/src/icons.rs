use bevy_egui::egui::{self, Color32, Painter, Pos2, Rect, Stroke, StrokeKind, vec2};

use crate::theme;

#[derive(Clone, Copy)]
pub(crate) enum Icon {
    More,
    Sort,
    Filter,
    View,
    Pause,
    Stop,
    Terminal,
    Clock,
    Tree,
    Graph,
    Inspector,
    Folder,
    Entity,
    Player,
}

pub(crate) fn paint(p: &Painter, c: Pos2, icon: Icon, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    let point = |x, y| c + vec2(x, y);
    let line = |points: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            points.iter().map(|&(x, y)| point(x, y)).collect(),
            stroke,
        ));
    };
    let outline = |x, y, w, h| {
        p.rect_stroke(
            Rect::from_center_size(point(x, y), vec2(w, h)),
            2,
            stroke,
            StrokeKind::Inside,
        );
    };
    match icon {
        Icon::More => {
            for y in [-4.0, 0.0, 4.0] {
                p.circle_filled(point(0.0, y), 1.0, color);
            }
        }
        Icon::Sort => {
            line(&[(-4.0, -2.0), (-1.5, -4.5), (1.0, -2.0)]);
            line(&[(-1.5, -4.5), (-1.5, 4.5)]);
            line(&[(4.0, 2.0), (1.5, 4.5), (-1.0, 2.0)]);
            line(&[(1.5, 4.5), (1.5, -4.5)]);
        }
        Icon::Filter => line(&[
            (-5.0, -4.0),
            (5.0, -4.0),
            (1.0, 0.5),
            (1.0, 4.0),
            (-1.0, 3.0),
            (-1.0, 0.5),
            (-5.0, -4.0),
        ]),
        Icon::View => {
            outline(0.0, -1.0, 13.0, 9.0);
            line(&[(-2.5, 5.5), (2.5, 5.5)]);
        }
        Icon::Pause => {
            line(&[(-2.0, -4.0), (-2.0, 4.0)]);
            line(&[(2.0, -4.0), (2.0, 4.0)]);
        }
        Icon::Stop => outline(0.0, 0.0, 8.0, 8.0),
        Icon::Terminal => {
            line(&[(-5.0, -3.0), (-2.0, 0.0), (-5.0, 3.0)]);
            line(&[(0.0, 3.0), (5.0, 3.0)]);
        }
        Icon::Clock => {
            p.circle_stroke(c, 5.5, stroke);
            line(&[(0.0, -3.0), (0.0, 0.0), (2.5, 1.5)]);
        }
        Icon::Tree | Icon::Inspector => {
            for y in [-4.0, 0.0, 4.0] {
                p.circle_filled(point(-5.0, y), 0.85, color);
                line(&[
                    (
                        if matches!(icon, Icon::Tree) && y != -4.0 {
                            -1.0
                        } else {
                            -2.0
                        },
                        y,
                    ),
                    (5.0, y),
                ]);
            }
        }
        Icon::Graph => {
            line(&[(-4.0, 3.5), (0.0, -3.5), (4.0, 3.5)]);
            for (x, y) in [(-4.0, 3.5), (0.0, -3.5), (4.0, 3.5)] {
                p.circle_filled(point(x, y), 2.0, color);
            }
        }
        Icon::Folder => line(&[
            (-6.0, 4.5),
            (-6.0, -4.5),
            (-1.5, -4.5),
            (0.5, -2.5),
            (6.0, -2.5),
            (6.0, 4.5),
            (-6.0, 4.5),
        ]),
        Icon::Entity => outline(0.0, 0.0, 10.0, 10.0),
        Icon::Player => {
            p.circle_stroke(point(0.0, -3.0), 2.2, stroke);
            line(&[(-4.0, 4.5), (-3.0, 1.8), (3.0, 1.8), (4.0, 4.5)]);
        }
    }
}

pub(crate) fn button(ui: &mut egui::Ui, glyph: Icon, active: bool, label: &str) -> egui::Response {
    let response = ui.add(
        egui::Button::new("")
            .selected(active)
            .frame_when_inactive(active)
            .min_size(vec2(24.0, theme::ROW)),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), active, label)
    });
    paint(
        ui.painter(),
        response.rect.center(),
        glyph,
        if response.hovered() || active {
            theme::TEXT
        } else {
            theme::SECONDARY
        },
    );
    response.on_hover_text(label)
}
