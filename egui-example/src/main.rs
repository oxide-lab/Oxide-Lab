#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;

fn main() -> eframe::Result {
    env_logger::init();
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([450.0, 400.0])
            .with_title("Oxide Lab - egui Example"),
        ..Default::default()
    };
    
    eframe::run_native(
        "Oxide Lab egui Example",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::<ChatApp>::default())
        }),
    )
}

#[derive(Default)]
struct ChatApp {
    name: String,
    message: String,
    messages: Vec<String>,
    slider_value: f32,
    show_modal: bool,
}

impl eframe::App for ChatApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top panel
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🦀 Oxide Lab - egui Demo");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::widgets::global_theme_preference_switch(ui);
                });
            });
        });

        // Central panel
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Simple Chat Interface");
            
            ui.separator();
            
            // Name input
            ui.horizontal(|ui| {
                ui.label("Your name:");
                ui.text_edit_singleline(&mut self.name);
            });
            
            // Slider
            ui.add(egui::Slider::new(&mut self.slider_value, 0.0..=100.0).text("Value"));
            
            ui.separator();
            
            // Message history
            ui.label(egui::RichText::new("Messages:").strong());
            egui::ScrollArea::vertical()
                .max_height(150.0)
                .show(ui, |ui| {
                    for msg in &self.messages {
                        ui.label(msg);
                    }
                });
            
            ui.separator();
            
            // Message input
            ui.horizontal(|ui| {
                ui.label("Message:");
                let response = ui.text_edit_singleline(&mut self.message);
                
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.send_message();
                }
                
                if ui.button("Send").clicked() {
                    self.send_message();
                }
            });
            
            ui.separator();
            
            // Buttons
            ui.horizontal(|ui| {
                if ui.button("Open Modal").clicked() {
                    self.show_modal = true;
                }
                
                if ui.button("Clear Messages").clicked() {
                    self.messages.clear();
                }
            });
        });

        // Modal dialog
        if self.show_modal {
            egui::Window::new("Confirmation")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("This is a modal dialog!");
                    ui.label(format!("Slider value: {:.1}", self.slider_value));
                    
                    ui.separator();
                    
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() {
                            self.show_modal = false;
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_modal = false;
                        }
                    });
                });
        }
    }
}

impl ChatApp {
    fn send_message(&mut self) {
        if !self.message.is_empty() {
            let name = if self.name.is_empty() {
                "Anonymous"
            } else {
                &self.name
            };
            self.messages.push(format!("{}: {}", name, self.message));
            self.message.clear();
        }
    }
}
