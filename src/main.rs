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

struct NotesApp {
    notes: Vec<Note>,
    selected: Option<usize>,
    search: String,
    data_path: String,
    dirty: bool,
}

impl Default for NotesApp {
    fn default() -> Self {
        let data_path = get_data_path();
        let notes = load_notes(&data_path).unwrap_or_default();
        let selected = if notes.is_empty() { None } else { Some(0) };
        Self {
            notes,
            selected,
            search: String::new(),
            data_path,
            dirty: false,
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
                self.selected = (0..self.notes.len()).next();
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
}

impl eframe::App for NotesApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        static mut FONT_SET: bool = false;
        unsafe {
            if !FONT_SET {
                let mut style = (*ctx.style()).clone();

                style.text_styles.get_mut(&egui::TextStyle::Body).unwrap().size = 17.0;
                style.text_styles.get_mut(&egui::TextStyle::Heading).unwrap().size = 24.0;
                style.text_styles.get_mut(&egui::TextStyle::Button).unwrap().size = 15.0;

                ctx.set_style(style);
                FONT_SET = true;
            }
        }

        egui::TopBottomPanel::top("top_panel")
            .frame(egui::Frame::default()
                .fill(ctx.style().visuals.panel_fill)
                .inner_margin(egui::Margin { top: 10.0, bottom: 10.0, left: 10.0, right: 10.0 })
                .stroke(egui::Stroke::new(0.0, egui::Color32::TRANSPARENT))
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("NOTES ");
                    if ui.button("New").clicked() {
                        self.add_note();
                    }
                    /*if ui.button("Save").clicked() {
                        self.save_notes();
                    }*/
                    if ui.button("Delete").clicked() {
                        self.delete_selected();
                    }
                });
            });

        egui::SidePanel::left("left_panel")
            .frame(egui::Frame::default()
                .fill(ctx.style().visuals.panel_fill)
                .inner_margin(egui::Margin { top: 10.0, bottom: 10.0, left: 10.0, right: 10.0 })
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

                    let mut to_select: Option<usize> = None;
                    for (i, note) in self
                        .notes
                        .iter()
                        .enumerate()
                        .filter(|(_, n)| {
                            let q = self.search.to_lowercase();
                            q.is_empty()
                                || n.title.to_lowercase().contains(&q)
                                || n.body.to_lowercase().contains(&q)
                        })
                    {
                        let selected = Some(i) == self.selected;
                        if ui
                            .selectable_label(selected, format!("{}", &note.title))
                            .clicked()
                        {
                            to_select = Some(i);
                        }
                    }

                    if let Some(s) = to_select {
                        self.selected = Some(s);
                    }

                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        ui.label(format!("{} notes", self.notes.len()));
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::default()
                .fill(ctx.style().visuals.panel_fill)
                .inner_margin(egui::Margin { top: 10.0, bottom: 10.0, left: 10.0, right: 15.0 })
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
                                    self.dirty = true;
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
                            if ui
                                .add(egui::TextEdit::multiline(&mut note.body).desired_rows(0).min_size(egui::vec2(0.0, available_height * 0.7)).desired_width(450.0))
                                .changed()
                            {
                                note.modified = current_unix();
                                self.dirty = true;
                            }
                        } else {
                            ui.label(&note.body);
                        }

                        ui.separator();

                        let mut save_clicked = false;
                        let last_modified = note.modified;
                        
                        let dt: DateTime<Local> = Local.timestamp_opt(last_modified as i64, 0).unwrap();
                        ui.label(
                            egui::RichText::new(format!("Last modified: {}", dt.format("  %d-%m-%Y   %H:%M")))
                                .size(10.0)
                        );
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
                                        ui.ctx().output_mut(|o| o.copied_text = note.body.clone());
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

        if self.dirty {
            self.save_notes();
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

fn main() {
    let native_options = eframe::NativeOptions::default();

    let _ = eframe::run_native(
        "Notes",
        native_options,
        Box::new(|_cc| Ok(Box::new(NotesApp::default()))),
    );
}