#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use eframe::egui;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use chrono::{DateTime, Local, TimeZone};

#[derive(Serialize, Deserialize, Clone)]
struct Note {
    id: u128,
    title: String,
    body: String,
    modified: u64,
    editing: bool,
    backup: Option<String>,
}

impl Note {
    fn new(id: u128) -> Self {
        Self {
            id,
            title: "Untitled".to_owned(),
            body: String::new(),
            modified: current_unix(),
            editing: false,
            backup: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct AppSettings {
    dark_mode: bool,
    font_size: f32,
    auto_save: bool,
    show_word_count: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            dark_mode: true,
            font_size: 17.0,
            auto_save: true,
            show_word_count: false,
        }
    }
}

#[derive(PartialEq)]
enum AppView {
    Notes,
    Settings,
}

fn current_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn get_data_path() -> String {
    let mut path = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    path.push("notes");
    let _ = std::fs::create_dir_all(&path);
    path.push("notes.json");
    path.to_string_lossy().to_string()
}

fn get_settings_path() -> String {
    let mut path = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    path.push("notes");
    let _ = std::fs::create_dir_all(&path);
    path.push("settings.json");
    path.to_string_lossy().to_string()
}

struct NotesApp {
    notes: Vec<Note>,
    selected: Option<usize>,
    search: String,
    data_path: String,
    settings_path: String,
    settings: AppSettings,
    dirty: bool,
    dragging: Option<usize>,
    drag_start_pos: Option<egui::Pos2>,
    current_view: AppView,
    settings_changed: bool,
}

impl Default for NotesApp {
    fn default() -> Self {
        let data_path = get_data_path();
        let settings_path = get_settings_path();
        let notes = load_notes(&data_path).unwrap_or_default();
        let settings = load_settings(&settings_path).unwrap_or_default();
        let selected = if notes.is_empty() { None } else { Some(0) };
        Self {
            notes,
            selected,
            search: String::new(),
            data_path,
            settings_path,
            settings,
            dirty: false,
            dragging: None,
            drag_start_pos: None,
            current_view: AppView::Notes,
            settings_changed: false,
        }
    }
}

impl NotesApp {
    fn add_note(&mut self) {
        let id = rand::random::<u128>();
        let mut note = Note::new(id);
        note.title = format!("Note {}", self.notes.len() + 1);
        self.notes.insert(0, note);
        self.selected = Some(0);
        self.dirty = true;
    }

    fn delete_selected(&mut self) {
        if let Some(idx) = self.selected {
            if idx < self.notes.len() {
                self.notes.remove(idx);
                self.selected = if self.notes.is_empty() { None } else { Some(0.min(idx)) };
                self.dirty = true;
            }
        }
    }

    fn save_notes(&mut self) {
        if let Err(e) = save_notes(&self.data_path, &self.notes) {
            eprintln!("Failed to save notes: {}", e);
        } else {
            self.dirty = false;
        }
    }

    fn save_settings(&mut self) {
        if let Err(e) = save_settings(&self.settings_path, &self.settings) {
            eprintln!("Failed to save settings: {}", e);
        } else {
            self.settings_changed = false;
        }
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        if self.settings.dark_mode {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }
    }

    fn apply_font_settings(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        
        style.text_styles.get_mut(&egui::TextStyle::Body).unwrap().size = self.settings.font_size;
        style.text_styles.get_mut(&egui::TextStyle::Heading).unwrap().size = self.settings.font_size + 7.0;
        style.text_styles.get_mut(&egui::TextStyle::Button).unwrap().size = self.settings.font_size - 2.0;
        
        ctx.set_style(style);
    }

    fn move_note(&mut self, from: usize, to: usize) {
        if from != to && from < self.notes.len() && to < self.notes.len() {
            let note = self.notes.remove(from);
            self.notes.insert(to, note);
            
            if let Some(selected_idx) = self.selected {
                if selected_idx == from {
                    self.selected = Some(to);
                } else if selected_idx > from && selected_idx <= to {
                    self.selected = Some(selected_idx - 1);
                } else if selected_idx < from && selected_idx >= to {
                    self.selected = Some(selected_idx + 1);
                }
            }
            
            self.dirty = true;
        }
    }

    fn show_settings_page(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.heading("Settings");
        ui.separator();
        ui.add_space(10.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.group(|ui| {
                ui.label(egui::RichText::new("Appearance").size(18.0));
                ui.add_space(5.0);
                
                ui.horizontal(|ui| {
                    ui.label("Theme:");
                    if ui.selectable_label(self.settings.dark_mode, "Dark").clicked() {
                        self.settings.dark_mode = true;
                        self.apply_theme(ctx);
                        self.settings_changed = true;
                    }
                    if ui.selectable_label(!self.settings.dark_mode, "Light").clicked() {
                        self.settings.dark_mode = false;
                        self.apply_theme(ctx);
                        self.settings_changed = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Font size:");
                    let mut font_size = self.settings.font_size;
                    if ui.add(egui::Slider::new(&mut font_size, 12.0..=24.0).step_by(1.0)).changed() {
                        self.settings.font_size = font_size;
                        self.apply_font_settings(ctx);
                        self.settings_changed = true;
                    }
                });
            });

            ui.add_space(10.0);

            ui.group(|ui| {
                ui.label(egui::RichText::new("Editor").size(18.0));
                ui.add_space(5.0);

                let mut auto_save = self.settings.auto_save;
                if ui.checkbox(&mut auto_save, "Auto-save notes").changed() {
                    self.settings.auto_save = auto_save;
                    self.settings_changed = true;
                }

                let mut show_word_count = self.settings.show_word_count;
                if ui.checkbox(&mut show_word_count, "Show word count").changed() {
                    self.settings.show_word_count = show_word_count;
                    self.settings_changed = true;
                }
            });

            ui.add_space(10.0);
            ui.group(|ui| {
                ui.label(egui::RichText::new("Storage Information").size(18.0));
                ui.add_space(5.0);
                
                ui.label("Notes stored at:");
                ui.label(format!("{}", self.data_path));
                ui.label("Settings stored at:");
                ui.label(format!("{}", self.settings_path));
                ui.label(format!("Total notes: {}", self.notes.len()));
            });

            ui.add_space(20.0);

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Reset to Defaults").clicked() {
                    self.settings = AppSettings::default();
                    self.apply_theme(ctx);
                    self.apply_font_settings(ctx);
                    self.settings_changed = true;
                }
            });
        });
    }

    fn get_word_count(text: &str) -> usize {
        text.split_whitespace().count()
    }
}

impl eframe::App for NotesApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        static mut THEME_APPLIED: bool = false;
        static mut FONT_SET: bool = false;

        unsafe {
            if !THEME_APPLIED {
                self.apply_theme(ctx);
                THEME_APPLIED = true;
            }

            if !FONT_SET {
                self.apply_font_settings(ctx);
                FONT_SET = true;
            }
        }

        egui::TopBottomPanel::top("top_panel")
            .frame(egui::Frame::default()
                .fill(ctx.style().visuals.panel_fill)
                .inner_margin(egui::Margin { top: 10, bottom: 10, left: 10, right: 10 })
                .stroke(egui::Stroke::new(0.0, egui::Color32::TRANSPARENT))
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.current_view == AppView::Notes, "Notes").clicked() {
                        self.current_view = AppView::Notes;
                    }
                    if ui.selectable_label(self.current_view == AppView::Settings, "Settings").clicked() {
                        self.current_view = AppView::Settings;
                    }

                    ui.separator();

                    if self.current_view == AppView::Notes {
                        if ui.button("New").clicked() {
                            self.add_note();
                        }
                        if ui.button("Delete").clicked() {
                            self.delete_selected();
                        }
                    }
                });
            });

        match self.current_view {
            AppView::Settings => {
                egui::CentralPanel::default()
                    .frame(egui::Frame::default()
                        .fill(ctx.style().visuals.panel_fill)
                        .inner_margin(egui::Margin { top: 10, bottom: 10, left: 20, right: 20 })
                        .stroke(egui::Stroke::new(0.0, egui::Color32::TRANSPARENT))
                    )
                    .show(ctx, |ui| {
                        self.show_settings_page(ctx, ui);
                    });
            }
            AppView::Notes => {
                egui::SidePanel::left("left_panel")
                    .frame(egui::Frame::default()
                        .fill(ctx.style().visuals.panel_fill)
                        .inner_margin(egui::Margin { top: 10, bottom: 10, left: 10, right: 10 })
                        .stroke(egui::Stroke::new(0.0, egui::Color32::TRANSPARENT))
                    )
                    .min_width(150.0).show(ctx, |ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label("Search:");
                                ui.text_edit_singleline(&mut self.search);
                            });
                            ui.add_space(2.0);
                            ui.separator();
                            ui.add_space(5.0);

                            let filtered_notes: Vec<(usize, String, u128)> = self
                                .notes
                                .iter()
                                .enumerate()
                                .filter(|(_, n)| {
                                    let q = self.search.to_lowercase();
                                    q.is_empty()
                                        || n.title.to_lowercase().contains(&q)
                                        || n.body.to_lowercase().contains(&q)
                                })
                                .map(|(i, n)| (i, n.title.clone(), n.id))
                                .collect();

                            let mut to_select: Option<usize> = None;
                            let mut move_from_to: Option<(usize, usize)> = None;

                            let enable_dnd = self.search.is_empty();

                            let line_height = ui.text_style_height(&egui::TextStyle::Body);
                            let spacing = ui.spacing().item_spacing.y;
                            let bottom_content_height = line_height + if enable_dnd { line_height + spacing } else { 0.0 } + spacing * 2.0;

                            let available_height = ui.available_height() - bottom_content_height;

                            egui::ScrollArea::vertical()
                                        .max_height(available_height)
                                        .show(ui, |ui| {
                                    for (_list_idx, (original_idx, title, _id)) in filtered_notes.iter().enumerate() {
                                        let selected = Some(*original_idx) == self.selected;
                                        
                                        if enable_dnd {
                                            let response = ui.allocate_response(
                                                egui::vec2(ui.available_width(), 20.0),
                                                egui::Sense::click_and_drag()
                                            );

                                            let label_response = ui.scope_builder(
                                                egui::UiBuilder::new().max_rect(response.rect),
                                                |ui| {
                                                    ui.selectable_label(selected, format!("{}", title))
                                                }
                                            ).inner;

                                            if label_response.clicked() {
                                                to_select = Some(*original_idx);
                                            }

                                            if response.drag_started() {
                                                self.dragging = Some(*original_idx);
                                                self.drag_start_pos = Some(response.interact_pointer_pos().unwrap_or_default());
                                            }

                                            if let Some(dragging_idx) = self.dragging {
                                                if dragging_idx == *original_idx {
                                                    let painter = ui.painter();
                                                    let drag_rect = response.rect;
                                                    
                                                    if let Some(pointer_pos) = ctx.pointer_latest_pos() {
                                                        if let Some(start_pos) = self.drag_start_pos {
                                                            let offset = pointer_pos - start_pos;
                                                            let dragged_rect = drag_rect.translate(egui::vec2(0.0, offset.y));
                                                            
                                                            painter.rect_filled(
                                                                dragged_rect,
                                                                4.0,
                                                                egui::Color32::from_rgba_premultiplied(30, 30, 30, 100)
                                                            );
                                                            painter.text(
                                                                dragged_rect.center(),
                                                                egui::Align2::CENTER_CENTER,
                                                                title,
                                                                egui::FontId::default(),
                                                                egui::Color32::WHITE
                                                            );
                                                        }
                                                    }

                                                    if let Some(pointer_pos) = ctx.pointer_latest_pos() {
                                                        for (_target_list_idx, (target_original_idx, _target_title, _target_id)) in filtered_notes.iter().enumerate() {
                                                            if *target_original_idx != dragging_idx {
                                                                let target_y = ui.min_rect().top() + (_target_list_idx as f32 * 25.0) + 40.0;
                                                                let target_rect = egui::Rect::from_min_size(
                                                                    egui::pos2(ui.min_rect().left(), target_y),
                                                                    egui::vec2(ui.available_width(), 20.0)
                                                                );
                                                                
                                                                if target_rect.contains(pointer_pos) {
                                                                    let painter = ui.painter();
                                                                    painter.hline(
                                                                        target_rect.x_range(),
                                                                        if pointer_pos.y < target_rect.center().y {
                                                                            target_rect.top()
                                                                        } else {
                                                                            target_rect.bottom()
                                                                        },
                                                                        egui::Stroke::new(2.0, egui::Color32::GRAY)
                                                                    );
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            if response.drag_stopped() && self.dragging.is_some() {
                                                if let Some(pointer_pos) = ctx.pointer_latest_pos() {
                                                    for (_target_list_idx, (target_original_idx, _target_title, _target_id)) in filtered_notes.iter().enumerate() {
                                                        if *target_original_idx != self.dragging.unwrap() {
                                                            let target_y = ui.min_rect().top() + (_target_list_idx as f32 * 25.0) + 40.0;
                                                            let target_rect = egui::Rect::from_min_size(
                                                                egui::pos2(ui.min_rect().left(), target_y),
                                                                egui::vec2(ui.available_width(), 20.0)
                                                            );
                                                            
                                                            if target_rect.contains(pointer_pos) {
                                                                move_from_to = Some((self.dragging.unwrap(), *target_original_idx));
                                                                break;
                                                            }
                                                        }
                                                    }
                                                }
                                                self.dragging = None;
                                                self.drag_start_pos = None;
                                            }
                                        } else {
                                            if ui
                                                .selectable_label(selected, format!("{}", title))
                                                .clicked()
                                            {
                                                to_select = Some(*original_idx);
                                            }
                                        }
                                    }
                                });

                            if let Some((from, to)) = move_from_to {
                                self.move_note(from, to);
                            }

                            if let Some(s) = to_select {
                                self.selected = Some(s);
                            }

                            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                                ui.label(format!("{} notes", self.notes.len()));
                                if enable_dnd {
                                    ui.label(egui::RichText::new("Drag to reorder").size(10.0));
                                }
                                ui.separator();
                            });
                        });
                    });

                egui::CentralPanel::default()
                    .frame(egui::Frame::default()
                        .fill(ctx.style().visuals.panel_fill)
                        .inner_margin(egui::Margin { top: 10, bottom: 10, left: 10, right: 15 })
                        .stroke(egui::Stroke::new(0.0, egui::Color32::TRANSPARENT))
                    )
                    .show(ctx, |ui| {
                        if let Some(idx) = self.selected {
                            if idx < self.notes.len() {
                                let note = &mut self.notes[idx];

                                if note.editing {
                                    ui.horizontal(|ui| {
                                        ui.label("Title:");
                                        if ui.text_edit_singleline(&mut note.title).changed() {
                                            note.modified = current_unix();
                                            if self.settings.auto_save {
                                                self.dirty = true;
                                            }
                                        }
                                    });
                                } else {
                                    ui.horizontal(|ui| {
                                        ui.label("");
                                        ui.label(egui::RichText::new(&note.title).heading());
                                    });
                                }

                                ui.separator();

                                if note.editing {
                                    ui.label("Body:");
                                    let available_height = ui.available_height();
                                    egui::ScrollArea::vertical()
                                        .max_height(available_height * 0.7)
                                        .show(ui, |ui| {
                                            if ui
                                                .add(egui::TextEdit::multiline(&mut note.body)
                                                    .desired_rows(0)
                                                    .desired_width(450.0))
                                                .changed()
                                            {
                                                note.modified = current_unix();
                                                if self.settings.auto_save {
                                                    self.dirty = true;
                                                }
                                            }
                                        });
                                } else {
                                    let available_height = ui.available_height();
                                    egui::ScrollArea::vertical()
                                        .max_height(available_height * 0.7)
                                        .show(ui, |ui| {
                                            ui.label(&note.body);
                                        });
                                }

                                ui.separator();

                                let mut save_clicked = false;
                                let last_modified = note.modified;
                                
                                let dt: DateTime<Local> = Local.timestamp_opt(last_modified as i64, 0).unwrap();
                                
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!("Last modified: {}", dt.format("%d-%m-%Y %H:%M")))
                                            .size(10.0)
                                    );
                                    
                                    if self.settings.show_word_count {
                                        let word_count = Self::get_word_count(&note.body);
                                        ui.label(
                                            egui::RichText::new(format!("Words: {}", word_count))
                                                .size(10.0)
                                        );
                                    }
                                });
                                
                                ui.horizontal(|ui| {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if note.editing {
                                            if ui.button("Save").clicked() {
                                                note.modified = current_unix();
                                                note.editing = false;
                                                save_clicked = true;
                                                note.backup = None;
                                            }
                                            if ui.button("Close").clicked() {
                                                if let Some(original) = &note.backup {
                                                    note.body = original.clone();
                                                }
                                                note.editing = false;
                                                note.backup = None;
                                            }
                                        } else {
                                            if ui.button("Edit").clicked() {
                                                note.backup = Some(note.body.clone());
                                                note.editing = true;
                                            }
                                            if ui.button("Copy").clicked() {
                                                ui.ctx().copy_text(note.body.clone());
                                            }
                                        }
                                    });
                                });

                                if save_clicked {
                                    self.dirty = true;
                                    self.save_notes();
                                }
                            }
                        } else {
                            ui.label("No note selected — create one with New");
                        }
                    });
            }
        }

        if self.dirty && self.settings.auto_save {
            self.save_notes();
        }

        if self.settings_changed {
            self.save_settings();
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.dragging.is_some() {
            self.dragging = None;
            self.drag_start_pos = None;
        }
    }
}

fn load_notes<P: AsRef<Path>>(path: P) -> Result<Vec<Note>, Box<dyn std::error::Error>> {
    if !path.as_ref().exists() {
        return Ok(vec![]);
    }
    let data = fs::read_to_string(path)?;
    let notes: Vec<Note> = serde_json::from_str(&data)?;
    Ok(notes)
}

fn save_notes<P: AsRef<Path>>(path: P, notes: &Vec<Note>) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(notes)?;
    fs::write(path, json)?;
    Ok(())
}

fn load_settings<P: AsRef<Path>>(path: P) -> Result<AppSettings, Box<dyn std::error::Error>> {
    if !path.as_ref().exists() {
        return Ok(AppSettings::default());
    }
    let data = fs::read_to_string(path)?;
    let settings: AppSettings = serde_json::from_str(&data)?;
    Ok(settings)
}

fn save_settings<P: AsRef<Path>>(path: P, settings: &AppSettings) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(settings)?;
    fs::write(path, json)?;
    Ok(())
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions::default();

    eframe::run_native(
        "Notes",
        native_options,
        Box::new(|_cc| Ok(Box::new(NotesApp::default()))),
    )
}