use std::{
    collections::VecDeque,
    fmt,
    sync::{Arc, Mutex},
    time::Instant,
};

use bevy::{
    log::{
        BoxedLayer, Level,
        tracing::{
            Event,
            field::{Field, Visit},
        },
        tracing_subscriber::{Layer, Registry, layer::Context},
    },
    prelude::{App, Resource},
};
use bevy_egui::egui::{self, RichText};

use crate::{
    icons::{self, Icon},
    theme,
};

pub(crate) const SESSION_TARGET: &str = "editor::session";
const CAPACITY: usize = 500;

#[derive(Clone)]
struct Entry {
    elapsed: f64,
    level: Level,
    target: &'static str,
    message: String,
    parameters: String,
}

struct Buffer {
    start: Instant,
    entries: VecDeque<Entry>,
}

impl Buffer {
    fn push(&mut self, entry: Entry) {
        if self.entries.len() == CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }
}

#[derive(Clone, Resource)]
pub(crate) struct LogBuffer(Arc<Mutex<Buffer>>);

pub(crate) fn log_layer(app: &mut App) -> Option<BoxedLayer> {
    let buffer = LogBuffer(Arc::new(Mutex::new(Buffer {
        start: Instant::now(),
        entries: VecDeque::with_capacity(CAPACITY),
    })));
    app.insert_resource(buffer.clone());
    Some(Box::new(buffer))
}

#[derive(Default)]
struct Fields {
    message: String,
    parameters: String,
}

impl Visit for Fields {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        use fmt::Write;
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            if !self.parameters.is_empty() {
                self.parameters.push_str(" · ");
            }
            let _ = write!(self.parameters, "{}={value:?}", field.name());
        }
    }
}

impl Layer<Registry> for LogBuffer {
    fn on_event(&self, event: &Event<'_>, _: Context<'_, Registry>) {
        let metadata = event.metadata();
        let mut fields = Fields::default();
        event.record(&mut fields);
        if let Ok(mut buffer) = self.0.lock() {
            let elapsed = buffer.start.elapsed().as_secs_f64();
            buffer.push(Entry {
                elapsed,
                level: *metadata.level(),
                target: metadata.target(),
                message: fields.message,
                parameters: fields.parameters,
            });
        }
    }
}

#[derive(Default)]
pub(crate) struct View {
    timeline: bool,
    search: String,
    level: Option<Level>,
    paused_scroll: bool,
}

impl View {
    pub(crate) fn draw(&mut self, ui: &mut egui::Ui, logs: &LogBuffer) {
        ui.horizontal(|ui| {
            for (timeline, icon, label) in [
                (false, Icon::Terminal, "Console"),
                (true, Icon::Clock, "Timeline"),
            ] {
                let selected = self.timeline == timeline;
                let mut title = egui::text::LayoutJob::default();
                title.append(
                    label,
                    17.0,
                    egui::TextFormat::simple(
                        egui::FontId::proportional(12.0),
                        if selected {
                            theme::TEXT
                        } else {
                            theme::SECONDARY
                        },
                    ),
                );
                let button =
                    ui.add(egui::Button::selectable(selected, title).frame_when_inactive(selected));
                icons::paint(
                    ui.painter(),
                    egui::pos2(button.rect.left() + 15.0, button.rect.center().y),
                    icon,
                    if selected {
                        theme::RUST
                    } else {
                        theme::SECONDARY
                    },
                );
                if button.clicked() {
                    self.timeline = timeline;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    theme::label(if self.paused_scroll {
                        "SCROLL OFF"
                    } else {
                        "LIVE"
                    })
                    .small(),
                );
            });
        });
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .desired_width((ui.available_width() - 174.0).max(45.0))
                    .min_size(egui::vec2(0.0, theme::ROW))
                    .hint_text(RichText::new("Search…").color(theme::MUTED)),
            );
            if ui
                .button("Limpar")
                .on_hover_text("Limpar registros do editor")
                .clicked()
                && let Ok(mut buffer) = logs.0.lock()
            {
                buffer.entries.clear();
            }
            if icons::button(
                ui,
                Icon::Pause,
                self.paused_scroll,
                "Pausar rolagem automática",
            )
            .clicked()
            {
                self.paused_scroll = !self.paused_scroll;
            }
            egui::ComboBox::from_id_salt("log_level")
                .width(50.0)
                .selected_text(self.level.map_or("Nível".into(), |l| l.to_string()))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.level, None, "Todos");
                    for level in [
                        Level::ERROR,
                        Level::WARN,
                        Level::INFO,
                        Level::DEBUG,
                        Level::TRACE,
                    ] {
                        ui.selectable_value(&mut self.level, Some(level), level.to_string());
                    }
                });
        });
        ui.add_space(4.0);
        let query = self.search.trim().to_lowercase();
        let entries: Vec<_> = logs
            .0
            .lock()
            .map(|buffer| {
                buffer
                    .entries
                    .iter()
                    .filter(|entry| {
                        (!self.timeline || entry.target == SESSION_TARGET)
                            && self.level.is_none_or(|level| level == entry.level)
                            && (query.is_empty()
                                || entry.message.to_lowercase().contains(&query)
                                || entry.target.to_lowercase().contains(&query)
                                || entry.parameters.to_lowercase().contains(&query))
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        egui::Frame::new()
            .fill(theme::DEEP)
            .corner_radius(theme::CARD_RADIUS)
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                let height = ui.available_height();
                ui.set_min_size(egui::vec2(ui.available_width(), height));
                ui.horizontal(|ui| {
                    ui.label(theme::label(if self.timeline { "EVENT" } else { "MESSAGE" }).small());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(theme::label("PARAMETERS").small());
                    });
                });
                ui.separator();
                if entries.is_empty() {
                    ui.label(theme::label(if self.timeline {
                        "Nenhum evento de simulação."
                    } else {
                        "Nenhum registro."
                    }));
                }
                egui::ScrollArea::vertical()
                    .max_height(ui.available_height())
                    .auto_shrink([false, false])
                    .stick_to_bottom(!self.paused_scroll)
                    .show_rows(ui, theme::ROW, entries.len(), |ui, range| {
                        for index in range {
                            let entry = &entries[index];
                            ui.allocate_ui_with_layout(
                                egui::vec2(ui.available_width(), theme::ROW),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    let (marker, _) = ui.allocate_exact_size(
                                        egui::vec2(10.0, theme::ROW),
                                        egui::Sense::hover(),
                                    );
                                    let c = marker.center();
                                    let p = ui.painter();
                                    match entry.level {
                                        Level::ERROR => {
                                            for y in [-2.5, 2.5] {
                                                p.line_segment(
                                                    [
                                                        c + egui::vec2(-2.5, y),
                                                        c + egui::vec2(2.5, -y),
                                                    ],
                                                    egui::Stroke::new(1.5, theme::ERROR),
                                                );
                                            }
                                        }
                                        Level::WARN => {
                                            p.add(egui::Shape::convex_polygon(
                                                [(-3.5, 2.5), (0.0, -3.5), (3.5, 2.5)]
                                                    .map(|(x, y)| c + egui::vec2(x, y))
                                                    .to_vec(),
                                                theme::WARNING,
                                                egui::Stroke::NONE,
                                            ));
                                        }
                                        _ if index + 1 == entries.len() && !self.timeline => {
                                            p.circle_filled(c, 2.5, theme::RUST);
                                        }
                                        _ => {
                                            p.circle_filled(c, 1.0, theme::MUTED);
                                        }
                                    }
                                    let time = format!("{:.3}s", entry.elapsed);
                                    let message = if entry.message.is_empty() {
                                        entry.target
                                    } else {
                                        &entry.message
                                    };
                                    theme::left_label(
                                        ui,
                                        (ui.available_width() - 68.0).max(10.0),
                                        RichText::new(message).monospace(),
                                    )
                                    .on_hover_text(format!(
                                        "{}\n{}\n{}",
                                        entry.target, message, entry.parameters
                                    ));
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(theme::label(time).monospace())
                                                .on_hover_text(&entry.parameters);
                                        },
                                    );
                                },
                            );
                        }
                    });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracing_events_keep_session_target_level_and_fields() {
        use bevy::log::{tracing, tracing_subscriber::prelude::*};

        let mut app = App::new();
        let layer = log_layer(&mut app).unwrap();
        tracing::subscriber::with_default(Registry::default().with(layer), || {
            tracing::warn!(target: SESSION_TARGET, mode = "Pause", "Teste da Timeline");
        });
        let buffer = app.world().resource::<LogBuffer>().0.lock().unwrap();
        let entry = buffer.entries.front().unwrap();
        assert_eq!(entry.level, Level::WARN);
        assert_eq!(entry.target, SESSION_TARGET);
        assert_eq!(entry.message, "Teste da Timeline");
        assert!(entry.parameters.contains("mode=\"Pause\""));
    }

    #[test]
    fn buffer_drops_oldest_records() {
        let mut buffer = Buffer {
            start: Instant::now(),
            entries: VecDeque::new(),
        };
        for n in 0..=CAPACITY {
            buffer.push(Entry {
                elapsed: n as f64,
                level: Level::INFO,
                target: SESSION_TARGET,
                message: String::new(),
                parameters: String::new(),
            });
        }
        assert_eq!(buffer.entries.len(), CAPACITY);
        assert_eq!(buffer.entries.front().unwrap().elapsed, 1.0);
    }
}
