use crate::{model, theme, widgets};
use eframe::egui::{
    self, Align, Color32, Frame, Layout, Margin, RichText, ScrollArea, Sense, Stroke,
};

pub struct CordsApp {
    workspaces: Vec<model::Workspace>,
    conversations: Vec<model::Conversation>,
    messages: Vec<model::Message>,
    selected_workspace: usize,
    selected_conversation: usize,
    search: String,
    draft: String,
    details_open: bool,
}

impl CordsApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);
        Self {
            workspaces: model::workspaces(),
            conversations: model::conversations(),
            messages: model::messages(),
            selected_workspace: 0,
            selected_conversation: 0,
            search: String::new(),
            draft: String::new(),
            details_open: true,
        }
    }

    fn rail(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("workspace_rail")
            .exact_width(72.0)
            .frame(
                Frame::new()
                    .fill(theme::RAIL)
                    .inner_margin(Margin::symmetric(10, 12)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    for (index, workspace) in self.workspaces.iter().enumerate() {
                        let selected = index == self.selected_workspace;
                        let response = Frame::new()
                            .fill(if selected {
                                theme::ACCENT
                            } else {
                                Color32::TRANSPARENT
                            })
                            .corner_radius(if selected { 16.0 } else { 25.0 })
                            .inner_margin(3.0)
                            .show(ui, |ui| {
                                widgets::avatar(ui, workspace.initials, workspace.color, 42.0, None)
                            })
                            .inner;
                        if response.on_hover_text(workspace.name).clicked() {
                            self.selected_workspace = index;
                        }
                        ui.add_space(7.0);
                    }
                    ui.separator();
                    ui.add_space(6.0);
                    if ui
                        .add_sized(
                            [44.0, 44.0],
                            egui::Button::new(RichText::new("+").size(25.0).color(theme::GREEN)),
                        )
                        .on_hover_text("Add space")
                        .clicked()
                    {}
                });
            });
    }

    fn conversation_sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("conversation_sidebar")
            .exact_width(292.0)
            .resizable(true)
            .width_range(250.0..=360.0)
            .frame(
                Frame::new()
                    .fill(theme::SIDEBAR)
                    .inner_margin(Margin::same(12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Messages");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let _ = widgets::pill(ui, "+ New", theme::ACCENT)
                            .on_hover_text("Start a conversation");
                    });
                });
                ui.add_space(10.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("⌕  Find a conversation")
                        .desired_width(f32::INFINITY)
                        .margin(egui::vec2(10.0, 8.0)),
                );
                widgets::section_label(ui, "Direct messages");
                let query = self.search.to_lowercase();
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (index, conversation) in self.conversations.iter().enumerate() {
                            if !query.is_empty()
                                && !conversation.name.to_lowercase().contains(&query)
                            {
                                continue;
                            }
                            let selected = index == self.selected_conversation;
                            let response = Frame::new()
                                .fill(if selected {
                                    theme::SURFACE_HOVER
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .corner_radius(8.0)
                                .inner_margin(Margin::symmetric(8, 8))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        widgets::avatar(
                                            ui,
                                            conversation.initials,
                                            conversation.color,
                                            38.0,
                                            Some(conversation.online),
                                        );
                                        ui.vertical(|ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    RichText::new(conversation.name)
                                                        .color(if selected {
                                                            theme::TEXT
                                                        } else {
                                                            theme::MUTED
                                                        })
                                                        .strong(),
                                                );
                                                if conversation.pinned {
                                                    ui.label(
                                                        RichText::new("◆")
                                                            .size(8.0)
                                                            .color(theme::FAINT),
                                                    );
                                                }
                                            });
                                            ui.label(
                                                RichText::new(conversation.preview)
                                                    .size(12.0)
                                                    .color(theme::FAINT),
                                            );
                                        });
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                if conversation.unread > 0 {
                                                    ui.label(
                                                        RichText::new(
                                                            conversation.unread.to_string(),
                                                        )
                                                        .size(11.0)
                                                        .color(Color32::WHITE)
                                                        .background_color(theme::RED),
                                                    );
                                                }
                                            },
                                        );
                                    });
                                })
                                .response
                                .interact(Sense::click());
                            if response.clicked() {
                                self.selected_conversation = index;
                            }
                            ui.add_space(2.0);
                        }
                    });
                ui.separator();
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    widgets::avatar(ui, "YO", theme::ACCENT, 38.0, Some(true));
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Avery Stone").strong());
                        ui.label(RichText::new("Available").size(12.0).color(theme::GREEN));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.small_button("⚙").on_hover_text("Settings");
                    });
                });
            });
    }

    fn details(&mut self, ctx: &egui::Context) {
        if !self.details_open {
            return;
        }
        let person = &self.conversations[self.selected_conversation];
        egui::SidePanel::right("details")
            .exact_width(286.0)
            .resizable(true)
            .width_range(250.0..=350.0)
            .frame(
                Frame::new()
                    .fill(theme::SIDEBAR)
                    .inner_margin(Margin::same(18)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Details");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .small_button("×")
                            .on_hover_text("Close details")
                            .clicked()
                        {
                            self.details_open = false;
                        }
                    });
                });
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    widgets::avatar(ui, person.initials, person.color, 78.0, Some(person.online));
                    ui.add_space(10.0);
                    ui.heading(person.name);
                    ui.label(
                        RichText::new(if person.online { "Online" } else { "Offline" }).color(
                            if person.online {
                                theme::GREEN
                            } else {
                                theme::FAINT
                            },
                        ),
                    );
                });
                ui.add_space(20.0);
                ui.separator();
                widgets::section_label(ui, "About");
                ui.label(
                    RichText::new(
                        "Product designer, serial note-taker, and enthusiastic houseplant keeper.",
                    )
                    .color(theme::MUTED),
                );
                widgets::section_label(ui, "Member since");
                ui.label(RichText::new("March 2024").color(theme::MUTED));
                widgets::section_label(ui, "Shared");
                Frame::new()
                    .fill(theme::SURFACE)
                    .corner_radius(9.0)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("▣").size(18.0).color(theme::ACCENT));
                            ui.vertical(|ui| {
                                ui.label(RichText::new("12 files").strong());
                                ui.label(
                                    RichText::new("Photos and documents")
                                        .size(12.0)
                                        .color(theme::FAINT),
                                );
                            });
                        });
                    });
                ui.add_space(8.0);
                Frame::new()
                    .fill(theme::SURFACE)
                    .corner_radius(9.0)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("⌁").size(18.0).color(theme::ACCENT));
                            ui.vertical(|ui| {
                                ui.label(RichText::new("8 links").strong());
                                ui.label(
                                    RichText::new("Shared in this chat")
                                        .size(12.0)
                                        .color(theme::FAINT),
                                );
                            });
                        });
                    });
            });
    }

    fn chat(&mut self, ctx: &egui::Context) {
        let person = self.conversations[self.selected_conversation].clone();
        egui::CentralPanel::default()
            .frame(Frame::new().fill(theme::CANVAS))
            .show(ctx, |ui| {
                Frame::new()
                    .fill(theme::SIDEBAR)
                    .inner_margin(Margin::symmetric(18, 10))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            widgets::avatar(
                                ui,
                                person.initials,
                                person.color,
                                36.0,
                                Some(person.online),
                            );
                            ui.vertical(|ui| {
                                ui.label(RichText::new(person.name).strong().color(theme::TEXT));
                                ui.label(
                                    RichText::new(if person.online {
                                        "Online"
                                    } else {
                                        "Last seen yesterday"
                                    })
                                    .size(12.0)
                                    .color(theme::FAINT),
                                );
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .button("ⓘ")
                                    .on_hover_text("Conversation details")
                                    .clicked()
                                {
                                    self.details_open = !self.details_open;
                                }
                                ui.button("⌕").on_hover_text("Search messages");
                            });
                        });
                    });

                let composer_height = 86.0;
                let available = (ui.available_height() - composer_height).max(100.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), available),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ScrollArea::vertical()
                            .stick_to_bottom(true)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.add_space(26.0);
                                ui.vertical_centered(|ui| {
                                    widgets::avatar(
                                        ui,
                                        person.initials,
                                        person.color,
                                        72.0,
                                        Some(person.online),
                                    );
                                    ui.heading(person.name);
                                    ui.label(
                                        RichText::new(format!(
                                            "This is the beginning of your conversation with {}.",
                                            person.name
                                        ))
                                        .color(theme::FAINT),
                                    );
                                });
                                ui.add_space(24.0);
                                ui.horizontal(|ui| {
                                    ui.add(egui::Separator::default().horizontal().spacing(8.0));
                                    ui.label(
                                        RichText::new("TODAY")
                                            .size(11.0)
                                            .color(theme::FAINT)
                                            .strong(),
                                    );
                                    ui.add(egui::Separator::default().horizontal().spacing(8.0));
                                });
                                ui.add_space(12.0);
                                for message in &self.messages {
                                    let response = Frame::new()
                                        .inner_margin(Margin::symmetric(18, 7))
                                        .show(ui, |ui| {
                                            ui.horizontal_top(|ui| {
                                                widgets::avatar(
                                                    ui,
                                                    message.initials,
                                                    message.color,
                                                    40.0,
                                                    None,
                                                );
                                                ui.vertical(|ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.label(
                                                            RichText::new(message.author)
                                                                .strong()
                                                                .color(if message.mine {
                                                                    theme::ACCENT
                                                                } else {
                                                                    theme::TEXT
                                                                }),
                                                        );
                                                        ui.label(
                                                            RichText::new(message.time)
                                                                .size(11.0)
                                                                .color(theme::FAINT),
                                                        );
                                                    });
                                                    ui.label(
                                                        RichText::new(&message.body)
                                                            .color(theme::MUTED),
                                                    );
                                                });
                                            });
                                        });
                                    if response.response.hovered() {
                                        ui.painter().rect_stroke(
                                            response.response.rect,
                                            6.0,
                                            Stroke::new(1.0_f32, theme::BORDER),
                                            egui::StrokeKind::Inside,
                                        );
                                    }
                                }
                                ui.add_space(14.0);
                            });
                    },
                );

                Frame::new()
                    .inner_margin(Margin::symmetric(18, 12))
                    .show(ui, |ui| {
                        Frame::new()
                            .fill(theme::SURFACE)
                            .corner_radius(10.0)
                            .inner_margin(Margin::symmetric(10, 4))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add_sized([30.0, 30.0], egui::Button::new("+"))
                                        .on_hover_text("Add attachment");
                                    let send = ui
                                        .add_sized(
                                            [ui.available_width() - 72.0, 42.0],
                                            egui::TextEdit::singleline(&mut self.draft)
                                                .hint_text(format!("Message {}", person.name))
                                                .frame(false),
                                        )
                                        .lost_focus()
                                        && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                    let clicked = ui
                                        .add_sized(
                                            [34.0, 34.0],
                                            egui::Button::new(
                                                RichText::new("➤").color(theme::ACCENT),
                                            ),
                                        )
                                        .on_hover_text("Send message")
                                        .clicked();
                                    if (send || clicked) && !self.draft.trim().is_empty() {
                                        self.messages.push(model::Message {
                                            author: "You",
                                            initials: "YO",
                                            color: theme::ACCENT,
                                            time: "Just now",
                                            body: self.draft.trim().to_owned(),
                                            mine: true,
                                        });
                                        self.draft.clear();
                                    }
                                });
                            });
                    });
            });
    }
}

impl eframe::App for CordsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.rail(ctx);
        self.conversation_sidebar(ctx);
        self.details(ctx);
        self.chat(ctx);
    }
}
