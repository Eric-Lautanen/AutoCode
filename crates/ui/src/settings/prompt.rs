use crate::helpers;
use crate::theme::Palette;
use autocode_core::state::AppState;
use egui::{RichText, TextEdit};

pub fn show_prompt(ui: &mut egui::Ui, state: &mut AppState) {
    helpers::section_heading(ui, "System Prompt");

    // Which project the box below edits, and whether that project has a prompt
    // of its own. Both are resolved before the box borrows `state` mutably.
    let editing = state.active_project().map(|p| p.name.clone());
    let has_own = state
        .active_project()
        .is_some_and(|p| autocode_core::storage::load_project_system_prompt(p).is_some());

    let context_line = match (&editing, has_own) {
        (Some(name), true) => format!("{name} has its own system prompt."),
        (Some(name), false) => format!(
            "{name} uses the default system prompt — editing it here gives this \
             project its own."
        ),
        (None, _) => "No project is open. Open a project to give it its own system \
                       prompt; the default below is what a new one starts from."
            .to_string(),
    };
    ui.label(
        RichText::new(context_line)
            .size(11.0)
            .color(Palette::TEXT_MUTED),
    );

    ui.label(
        RichText::new(
            "Injected as the first message of every new session in this project. \
             Sessions that already exist keep the prompt they were started with.",
        )
        .size(11.0)
        .color(Palette::TEXT_MUTED),
    );
    ui.add_space(8.0);

    let project_open = editing.is_some();
    let prompt_resp = ui.add_enabled(
        project_open,
        TextEdit::multiline(&mut state.system_prompt)
            .desired_rows(20)
            .desired_width(ui.available_width())
            .font(egui::TextStyle::Monospace)
            .text_color(Palette::TEXT_PRIMARY),
    );
    if prompt_resp.changed() {
        // Written to the project's meta.json when the settings window closes,
        // on the next autosave, or when the project changes — not once per
        // keystroke.
        state.system_prompt_dirty = true;
    }

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(project_open, egui::Button::new("Reset to Default"))
            .on_hover_text("Drops this project's own prompt so it inherits the default")
            .clicked()
        {
            state.system_prompt = state.default_system_prompt.clone();
            state.system_prompt_dirty = true;
        }
    });

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(8.0);

    // -- App-wide default prompt ------------------------------------------
    ui.label(
        RichText::new("Default System Prompt")
            .size(14.0)
            .strong()
            .color(Palette::TEXT_PRIMARY),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new(
            "Used by every project that has not been given a prompt of its own, \
             including new ones.",
        )
        .size(11.0)
        .color(Palette::TEXT_MUTED),
    );
    ui.add_space(8.0);

    let default_resp = ui.add(
        TextEdit::multiline(&mut state.default_system_prompt)
            .desired_rows(10)
            .desired_width(ui.available_width())
            .font(egui::TextStyle::Monospace)
            .text_color(Palette::TEXT_PRIMARY),
    );
    if default_resp.changed() {
        // A project that inherits this must follow it live; one with its own
        // prompt keeps it.
        state.refresh_inherited_system_prompt();
    }

    ui.add_space(8.0);
    if ui.button("Reset to Built-in Default").clicked() {
        state.default_system_prompt = autocode_core::state::DEFAULT_SYSTEM_PROMPT.to_string();
        state.refresh_inherited_system_prompt();
    }

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
