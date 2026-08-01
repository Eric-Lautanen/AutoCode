use crate::helpers;
use crate::theme::Palette;
use autocode_core::state::AppState;
use egui::{RichText, TextEdit};

pub fn show_prompt(ui: &mut egui::Ui, state: &mut AppState) {
    helpers::section_heading(ui, "System Prompt");

    ui.label(
        RichText::new("Injected as the first message of every new session.")
            .size(11.0)
            .color(Palette::TEXT_MUTED),
    );
    ui.add_space(8.0);

    ui.add(
        TextEdit::multiline(&mut state.system_prompt)
            .desired_rows(20)
            .desired_width(ui.available_width())
            .font(egui::TextStyle::Monospace)
            .text_color(Palette::TEXT_PRIMARY),
    );

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ui.button("Reset to Default").clicked() {
            state.system_prompt = autocode_core::state::DEFAULT_SYSTEM_PROMPT.to_string();
        }
    });

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(8.0);

    // -- Handoff trigger prompt ------------------------------------------
    ui.label(
        RichText::new("Handoff Trigger Prompt")
            .size(14.0)
            .strong()
            .color(Palette::TEXT_PRIMARY),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Sent as a user message when the context threshold is reached and the \
             model hasn't called handoff. Instructs the model to stop work, record \
             tasks, and hand off with a generic next_prompt for the new session \
             (read the README and project docs, then continue).",
        )
        .size(11.0)
        .color(Palette::TEXT_MUTED),
    );
    ui.add_space(8.0);

    ui.add(
        TextEdit::multiline(&mut state.handoff_trigger_prompt)
            .desired_rows(6)
            .desired_width(ui.available_width())
            .font(egui::TextStyle::Monospace)
            .text_color(Palette::TEXT_PRIMARY),
    );

    ui.add_space(8.0);
    if ui.button("Reset to Default").clicked() {
        state.handoff_trigger_prompt =
            autocode_core::state::DEFAULT_HANDOFF_TRIGGER_PROMPT.to_string();
    }

    ui.add_space(16.0);

    // -- Handoff continuation prompt --------------------------------------
    ui.label(
        RichText::new("Handoff Continuation Prompt")
            .size(14.0)
            .strong()
            .color(Palette::TEXT_PRIMARY),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Injected as a synthetic user message before the project_task_list \
             tool call in a fresh handoff session. Tells the model to load and \
             review project tasks. The tool result + tasks are already visible \
             in the conversation by the time the model generates its response.",
        )
        .size(11.0)
        .color(Palette::TEXT_MUTED),
    );
    ui.add_space(8.0);

    ui.add(
        TextEdit::multiline(&mut state.handoff_continuation_prompt)
            .desired_rows(6)
            .desired_width(ui.available_width())
            .font(egui::TextStyle::Monospace)
            .text_color(Palette::TEXT_PRIMARY),
    );

    ui.add_space(8.0);
    if ui.button("Reset to Default").clicked() {
        state.handoff_continuation_prompt =
            autocode_core::state::DEFAULT_HANDOFF_CONTINUATION_PROMPT.to_string();
    }

    ui.add_space(16.0);

    // -- Handoff fallback prompt -----------------------------------------
    ui.label(
        RichText::new("Handoff Fallback Prompt")
            .size(14.0)
            .strong()
            .color(Palette::TEXT_PRIMARY),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "First message in a fresh session when a handoff happens without a \
             model-generated next_prompt — for example a forced handoff because \
             the context window would be exceeded.",
        )
        .size(11.0)
        .color(Palette::TEXT_MUTED),
    );
    ui.add_space(8.0);

    ui.add(
        TextEdit::multiline(&mut state.handoff_fallback_prompt)
            .desired_rows(4)
            .desired_width(ui.available_width())
            .font(egui::TextStyle::Monospace)
            .text_color(Palette::TEXT_PRIMARY),
    );

    ui.add_space(8.0);
    if ui.button("Reset to Default").clicked() {
        state.handoff_fallback_prompt =
            autocode_core::state::DEFAULT_HANDOFF_FALLBACK_PROMPT.to_string();
    }

    ui.add_space(16.0);

    // -- Loop warning prompt ---------------------------------------------
    ui.label(
        RichText::new("Loop Warning Prompt")
            .size(14.0)
            .strong()
            .color(Palette::TEXT_PRIMARY),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Injected as a user message when the model makes the exact same tool \
             call (same name and arguments) three turns in a row, signalling it \
             is stuck in a loop. The counters reset after firing, so the model \
             gets a fresh slate — three more identical turns will re-trigger it.",
        )
        .size(11.0)
        .color(Palette::TEXT_MUTED),
    );
    ui.add_space(8.0);

    ui.add(
        TextEdit::multiline(&mut state.loop_warning_prompt)
            .desired_rows(6)
            .desired_width(ui.available_width())
            .font(egui::TextStyle::Monospace)
            .text_color(Palette::TEXT_PRIMARY),
    );

    ui.add_space(8.0);
    if ui.button("Reset to Default").clicked() {
        state.loop_warning_prompt = autocode_core::state::DEFAULT_LOOP_WARNING_PROMPT.to_string();
    }

    ui.add_space(16.0);
}
