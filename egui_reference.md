# egui / eframe Quick Reference
**egui 0.34.2 · eframe 0.34.2** — Immediate mode GUI for native & web (Wasm)
MSRV: Rust 1.92.0. Font rendering uses `skrifa` + `vello_cpu` (replaced `ab_glyph` in 0.34). **wgpu is now the default renderer** (glow is opt-in). Released 2026-05-04.

> **Version history (recent):** 0.34.2 (2026-05-04) · 0.34.1 (2026-03-27) · 0.34.0 (2026-03-26) · 0.33.3 (2025-12-11) · 0.33.2 (2025-11-13) · 0.33.0 (2025-10-09) · 0.32.0 (2025-07-10)

---

## eframe — App Lifecycle

### Entry Points
- `eframe::run_native(title, NativeOptions, AppCreator)` — start a native desktop app
- `eframe::run_simple_native(title, NativeOptions, update_fn)` — simplest native entry point (no persistence)
- `eframe::create_native(NativeOptions, AppCreator)` — create app proxy for use on a custom event loop
- `WebRunner::new()` — construct a web runner handle
- `WebRunner::start(canvas, WebOptions, AppCreator)` — start app on a web canvas (async)
- `WebRunner::destroy()` — shut down the web app and free resources
- `WebRunner::app_mut::<T>()` — get mutable reference to your app from JS
- `WebRunner::has_panicked()` — check if the app has panicked
- `WebRunner::panic_summary()` — retrieve panic message and callstack

### `struct CreationContext`
Passed to your `AppCreator` closure; use it to initialize fonts, styles, and native resources once at startup.
- `cc.egui_ctx` — the `egui::Context`
- `cc.integration_info` — `IntegrationInfo` (web flag, system theme, etc.)
- `cc.storage` — `Option<&dyn Storage>` — read persisted state from previous session
- `cc.gl` — `Option<Arc<glow::Context>>` — OpenGL context (glow backend only)
- `cc.wgpu_render_state` — `Option<RenderState>` — wgpu render state (wgpu backend only)

### `trait App`
- `App::ui(&mut self, ui: &mut Ui, frame: &mut Frame)` — **required**; called each repaint; the new primary entry point (0.34+)
- `App::logic(&mut self, ctx: &Context, frame: &mut Frame)` — called before `ui()`, also when UI is hidden; do NOT paint here
- `App::update(&mut self, ctx: &Context, frame: &mut Frame)` — *(deprecated)* old entry point; use `ui` instead
- `App::save(&mut self, storage: &mut dyn Storage)` — called on shutdown and periodically to persist state
- `App::on_exit(&mut self, gl: Option<&glow::Context>)` — called once after `save()` on shutdown
- `App::auto_save_interval(&self) -> Duration` — override the auto-save interval
- `App::clear_color(&self, visuals: &Visuals) -> [f32; 4]` — background clear colour as linear sRGB
- `App::persist_egui_memory(&self) -> bool` — whether to persist egui memory (window positions etc.)
- `App::raw_input_hook(&mut self, ctx: &Context, raw_input: &mut RawInput)` — intercept raw input each frame

### `struct Frame`
- `Frame::info()` — returns `&IntegrationInfo` (screen size, system theme, web flag, etc.)
- `Frame::is_web()` — `true` when running in a browser (equivalent to `cfg!(target_arch="wasm32")`)
- `Frame::storage()` — `Option<&dyn Storage>` — read-only persistence storage
- `Frame::storage_mut()` — `Option<&mut dyn Storage>` — mutable persistence storage
- `Frame::gl()` — `Option<&Arc<glow::Context>>` — OpenGL context (glow feature + Renderer::Glow)
- `Frame::register_native_glow_texture(native)` — register a raw `glow::Texture` → `TextureId`
- `Frame::wgpu_render_state()` — `Option<&RenderState>` — wgpu render state (wgpu feature)
- `Frame::window_chrome_metrics()` — `Option<&WindowChromeMetrics>` — macOS title bar / traffic light insets (macOS only, 0.34+)
- `Frame` implements `HasWindowHandle` + `HasDisplayHandle` (native only) for raw window access

> **Window management** is done via `ctx.send_viewport_cmd(ViewportCommand::…)` — e.g. `ViewportCommand::Title`, `ViewportCommand::Resize`, `ViewportCommand::Fullscreen`, `ViewportCommand::Close`, `ViewportCommand::StartDrag`, `ViewportCommand::Minimized`, `ViewportCommand::Maximized`, `ViewportCommand::Decorations`, `ViewportCommand::Visible`, `ViewportCommand::OuterPosition`, `ViewportCommand::CancelClose`, `ViewportCommand::Transparent`, `ViewportCommand::WindowLevel`, `ViewportCommand::RequestUserAttention`, `ViewportCommand::SetTheme`, `ViewportCommand::ContentProtected`, `ViewportCommand::CursorPosition`, `ViewportCommand::CursorGrab`, `ViewportCommand::CursorVisible`, `ViewportCommand::MousePassthrough`, `ViewportCommand::Screenshot`.

> **Safe areas (iOS & notch support, 0.33+):** `ctx.screen_rect()` returns the full viewport rect; `ctx.content_rect()` returns the safe-area-inset rect (avoids notches, system UI). Prefer `content_rect` for placing widgets. `SafeAreaInsets` is exposed via `ViewportInfo`. Note: `screen_rect` was deprecated in 0.33 in favour of `content_rect`.

### `struct NativeOptions` (fields)
- `NativeOptions::viewport` — `ViewportBuilder` — window title, size, icon, decorations, etc.
- `NativeOptions::vsync` — enable vertical sync (default `true`)
- `NativeOptions::multisampling` — MSAA sample count (power of two; 0 = off)
- `NativeOptions::depth_buffer` — depth buffer bits (default 0)
- `NativeOptions::stencil_buffer` — stencil buffer bits (default 0)
- `NativeOptions::hardware_acceleration` — `HardwareAcceleration::{Preferred, Required, Off}`
- `NativeOptions::renderer` — `Renderer::Wgpu` (default in 0.34+) or `Renderer::Glow`
- `NativeOptions::run_and_return` — if `true`, execution continues after the window closes
- `NativeOptions::event_loop_builder` — `Option<EventLoopBuilderHook>` — customise the winit event loop
- `NativeOptions::window_builder` — `Option<WindowBuilderHook>` — customise the native window
- `NativeOptions::shader_version` — `Option<ShaderVersion>` — override GLSL version (glow backend)
- `NativeOptions::centered` — centre window on screen at startup (not supported on Wayland)
- `NativeOptions::wgpu_options` — `WgpuConfiguration` for the wgpu backend
- `NativeOptions::persist_window` — save/restore window position and size (persistence feature)
- `NativeOptions::persistence_path` — `Option<PathBuf>` — override the default app-state directory
- `NativeOptions::dithering` — apply dithering to reduce banding (default `true`)

> **Cargo.toml tip:** To use wgpu with its default backends (DX12/Vulkan/Metal/GL), add `wgpu = "25"` alongside `eframe = "0.34"`. For a slimmer binary, use the `wgpu_no_default_features` eframe feature and select backends manually.

### `struct WebOptions` (key fields)
- `WebOptions::wgpu_options` — wgpu options for web
- `WebOptions::default_theme` — light / dark default
- `WebOptions::follow_system_theme` — respect OS colour scheme
- `WebOptions::depth_buffer` — depth buffer bits on web canvas
- `WebOptions::renderer` — `Renderer::Wgpu` (default) or `Renderer::Glow`

### Persistence / Storage
- `eframe::get_value::<T>(storage, key)` — deserialise a RON value from storage
- `eframe::set_value(storage, key, value)` — serialise a value to storage as RON
- `eframe::storage_dir(app_id)` — path where eframe stores native app state
- `Storage::get_string(key)` — raw string get from storage
- `Storage::set_string(key, value)` — raw string set in storage
- `Storage::flush()` — force write storage to disk
- `APP_KEY` — the default storage key used for the top-level app

### Logging (web)
- `WebLogger::init(log::LevelFilter)` — redirect Rust `log` to `console.log`

---

## egui — Context

The `Context` is cheaply cloneable and `Send + Sync`. `Ui` implements `Deref<Target = Context>` (added 0.34), so all `ui.ctx().foo(…)` calls can be written as `ui.foo(…)` directly.

### Frame / Pass Control
- `ctx.run_ui(raw_input, |ui| …)` — **preferred** entry point (0.34+); runs one full egui frame with a top-level `Ui` covering `content_rect`; returns `FullOutput`
- `ctx.run(raw_input, |ctx| …)` — older entry point; still works but `run_ui` is preferred; returns `FullOutput`
- `ctx.begin_pass(raw_input)` — low-level: begin a pass
- `ctx.begin_frame(raw_input)` — *(deprecated)* renamed to `begin_pass`
- `ctx.end_pass()` / `ctx.end_frame()` — low-level: finish pass
- `ctx.tessellate(shapes, pixels_per_point)` — convert shapes to `ClippedPrimitive`s
- `ctx.request_repaint()` — schedule a repaint on the next frame
- `ctx.request_repaint_after(duration)` — repaint after a delay
- `ctx.request_repaint_after_secs(secs)` — repaint after N seconds
- `ctx.request_repaint_after_for(viewport_id, duration)` — per-viewport delayed repaint
- `ctx.request_repaint_of(viewport_id)` — repaint a specific viewport
- `ctx.request_discard(reason)` — discard this pass and redo it (multi-pass)
- `ctx.has_requested_repaint()` — query if repaint is pending
- `ctx.has_requested_repaint_for(viewport_id)` — per-viewport repaint pending
- `ctx.repaint_causes()` — list of reasons a repaint was requested
- `ctx.requested_repaint_last_pass()` — whether repaint was requested last pass
- `ctx.will_discard()` — whether current pass will be discarded

### Input
- `ctx.input(|i| …)` — read `InputState` (immutable closure)
- `ctx.input_mut(|i| …)` — mutate `InputState`
- `ctx.input_for(viewport_id, |i| …)` — input for a specific viewport
- `ctx.egui_wants_keyboard_input()` — true if egui is consuming keyboard events (renamed from `wants_keyboard_input` in 0.34)
- `ctx.egui_wants_pointer_input()` — true if egui is consuming pointer events (renamed from `wants_pointer_input` in 0.34)
- `ctx.egui_is_using_pointer()` — pointer is captured by egui (renamed from `is_using_pointer` in 0.34)
- `ctx.pointer_hover_pos()` — current hover position
- `ctx.pointer_latest_pos()` — most recent pointer position
- `ctx.multi_touch()` — multi-touch gesture info
- `ctx.on_begin_pass(callback)` — register a callback run at pass start
- `ctx.on_end_pass(callback)` — register a callback run at pass end

### Style, Visuals & Theme
- `ctx.style()` — *(deprecated 0.34 — renamed to `global_style()` to avoid confusion with `Ui::style()`)* read global `Style`
- `ctx.style_mut(|s| …)` — *(deprecated 0.34)* mutate global style; use `global_style_mut` instead
- `ctx.global_style()` / `ctx.global_style_mut(|s| …)` — global style shared across viewports (preferred as of 0.34)
- `ctx.style_of(theme)` — read `Style` for a specific `Theme` variant
- `ctx.style_mut_of(theme, |s| …)` — mutate style for a specific theme
- `ctx.set_style(style)` — replace the global style
- `ctx.set_style_of(theme, style)` — replace style for a specific theme
- `ctx.set_visuals(visuals)` — set visual appearance (dark/light theme colours)
- `ctx.set_visuals_of(theme, visuals)` — set visuals for a specific theme
- `ctx.set_theme(Theme)` — set `Theme::Dark` or `Theme::Light`
- `ctx.system_theme()` — OS-reported theme preference
- `ctx.theme()` — current effective theme
- `ctx.set_zoom_factor(f)` — scale all UI elements
- `ctx.zoom_factor()` — current zoom factor
- `ctx.set_pixels_per_point(f)` — override DPI scaling
- `ctx.pixels_per_point()` — current DPI scale
- `ctx.native_pixels_per_point()` — raw native DPI
- `ctx.set_fonts(font_definitions)` — install custom fonts
- `ctx.add_font(font_data)` — add a single font at runtime
- `ctx.fonts(|f| …)` — read font system
- `ctx.fonts_mut(|f| …)` — mutate font system
- `ctx.all_styles_mut(|s| …)` — apply changes to all viewport styles

### Viewports (multi-window)
- `ctx.show_viewport_immediate(id, builder, |ui, class| …)` — show a viewport synchronously; callback receives `&mut Ui` (0.34+; previously `&Context`)
- `ctx.show_viewport_deferred(id, builder, callback)` — show a viewport asynchronously; callback receives `&mut Ui`
- `ctx.viewport_id()` — id of the current viewport
- `ctx.parent_viewport_id()` — id of the parent viewport
- `ctx.viewport(|info| …)` — read `ViewportInfo` for current viewport
- `ctx.viewport_for(id, |info| …)` — read info for a specific viewport
- `ctx.viewport_rect()` — screen rect of current viewport
- `ctx.screen_rect()` — full screen rect (including areas under notches/system UI)
- `ctx.available_rect()` — *(deprecated 0.34)* area not covered by panels; use the panel `InnerResponse` rect instead
- `ctx.content_rect()` — safe-area inset rect; prefer this over `screen_rect` on iOS/notch devices
- `ctx.used_rect()` — rect used by content this frame
- `ctx.used_size()` — *(deprecated 0.34)* `Vec2` of used content size
- `ctx.send_viewport_cmd(cmd)` — send a `ViewportCommand` to the current viewport
- `ctx.send_viewport_cmd_to(id, cmd)` — send a command to any viewport
- `ctx.embed_viewports()` / `ctx.set_embed_viewports(bool)` — render child viewports inside parent
- `ctx.set_transform_layer(id, transform)` — apply a visual transform to a layer
- `ctx.transform_layer_shapes(id, transform)` — apply a transform to all shapes on a layer
- `ctx.layer_transform_to_global(layer_id, pos)` — map a position from layer to global space
- `ctx.layer_transform_from_global(layer_id, pos)` — map a position from global to layer space
- `ctx.layer_painter(layer_id)` — get a `Painter` for any layer (lower-level than `ui.painter()`)

### Memory & Data
- `ctx.memory(|m| …)` — read `Memory` (widget state, focus, popups)
- `ctx.memory_mut(|m| …)` — mutate memory
  - `memory.move_focus(direction)` — programmatically move keyboard focus (0.33+)
  - `memory.surrender_focus_on` — `SurrenderFocusOn` option controlling when focus is released (0.33+)
- `ctx.data(|d| …)` — read arbitrary typed data stored in context (`IdTypeMap`)
- `ctx.data_mut(|d| …)` — write arbitrary typed data
- `ctx.output(|o| …)` — read `PlatformOutput` (cursor icon, events, clipboard, etc.)
- `ctx.output_mut(|o| …)` — mutate `PlatformOutput`
- `ctx.graphics(|g| …)` — read `GraphicLayers` (painted shapes)
- `ctx.graphics_mut(|g| …)` — write to `GraphicLayers`
- `ctx.cumulative_frame_nr()` — total frames rendered
- `ctx.cumulative_frame_nr_for(viewport_id)` — per-viewport frame count
- `ctx.cumulative_pass_nr()` — total passes run
- `ctx.cumulative_pass_nr_for(viewport_id)` — per-viewport pass count
- `ctx.current_pass_index()` — index of the current pass within this frame (for multi-pass)

### Interaction & Hit-testing
- `ctx.read_response(id)` — read a widget's response before it is added
- `ctx.is_being_dragged(id)` — check if a widget is being dragged
- `ctx.dragged_id()` — id of the widget currently being dragged
- `ctx.drag_started_id()` / `ctx.drag_stopped_id()` — drag lifecycle ids
- `ctx.stop_dragging()` — cancel the current drag
- `ctx.dragging_something_else(id)` — something else is being dragged
- `ctx.set_dragged_id(id)` — forcibly set the dragged widget
- `ctx.layer_id_at(pos)` — layer under a screen position
- `ctx.top_layer_id()` — topmost layer id
- `ctx.rect_contains_pointer(layer, rect)` — hit-test a rect
- `ctx.is_pointer_over_egui()` — pointer is anywhere over any egui area; renamed from `is_pointer_over_area()` in 0.34 (was buggy before 0.34.2, now reliable)
- `ctx.egui_is_using_pointer()` — egui is consuming pointer input (renamed from `is_using_pointer` in 0.34)
- `ctx.interaction_snapshot(|s| …)` — detailed interaction data
- `ctx.highlight_widget(id)` — visually highlight a widget next frame

### Popups & Menus
- `ctx.is_popup_open(id)` — check if a popup is open
- `ctx.any_popup_open()` — any popup is currently open
- `ctx.is_context_menu_open()` — a context menu is open
- `ctx.move_to_top(layer_id)` — bring a layer to the front

### Textures & Images
- `ctx.load_texture(name, image, options)` — upload a `ColorImage` → `TextureHandle`
- `ctx.forget_image(uri)` — evict an image from the loader cache
- `ctx.forget_all_images()` — clear all cached images
- `ctx.has_pending_images()` — images are still loading
- `ctx.try_load_texture(uri, options, size)` — load a texture by URI
- `ctx.try_load_image(uri, size)` — load an image by URI
- `ctx.try_load_bytes(uri)` — load raw bytes by URI
- `ctx.add_bytes_loader(loader)` — register a custom bytes loader
- `ctx.add_image_loader(loader)` — register a custom image loader
- `ctx.add_texture_loader(loader)` — register a custom texture loader
- `ctx.is_loader_installed(type_id)` — check if a loader is registered
- `ctx.loaders()` — access the loader registry
- `ctx.include_bytes(uri, bytes)` — embed bytes directly into the loader cache
- `ctx.tex_manager()` — access the texture manager
- `ctx.copy_text(text)` — write text to the system clipboard
- `ctx.copy_image(image)` — write an image to the clipboard

### Accessibility
- `ctx.enable_accesskit()` / `ctx.disable_accesskit()` — toggle AccessKit
- `ctx.accesskit_node_builder(id)` — build an AccessKit node
- `ctx.register_widget_info(id, info)` — register widget info for accessibility

### Animation
- `ctx.animate_bool(id, bool)` — smooth 0→1 animation for a boolean
- `ctx.animate_bool_responsive(id, bool)` — faster animation
- `ctx.animate_bool_with_time(id, bool, secs)` — custom duration
- `ctx.animate_bool_with_easing(id, bool, easing)` — custom easing
- `ctx.animate_value_with_time(id, value, secs)` — animate an `f32`
- `ctx.clear_animations()` — remove all animation state

### Debug & Inspection
- `ctx.debug_on_hover()` / `ctx.set_debug_on_hover(bool)` — show widget debug info on hover
- `ctx.debug_text(pos, text)` — draw debug text at a screen position
- `ctx.debug_painter()` — painter for debug overlays
- `ctx.inspection_ui(ui)` — show egui internals panel
- `ctx.memory_ui(ui)` — show memory/state debug panel
- `ctx.style_ui(ui)` — show style editor panel
- `ctx.settings_ui(ui)` — show all settings panels
- `ctx.texture_ui(ui)` — show loaded textures panel
- `ctx.loaders_ui(ui)` — show loader state panel

### Plugins
- `ctx.add_plugin(plugin)` — install a context plugin (each type may only be registered once)
- `ctx.plugin::<T>()` — get a required plugin (panics if not installed)
- `ctx.plugin_opt::<T>()` — get an optional plugin
- `ctx.plugin_or_default::<T>()` — get plugin or insert default
- `ctx.with_plugin::<T>(|p| …)` — access plugin by closure

#### `trait Plugin` (0.33+)
Replaces the old `Context::on_begin_pass` / `Context::on_end_pass` callbacks with a structured trait-based API. State lives on the plugin struct itself, persisting across frames.
- `Plugin::debug_name(&self) -> &'static str` — **required**; used for profiling labels
- `Plugin::setup(&mut self, ctx: &Context)` — called once when the plugin is registered (good place to install image loaders)
- `Plugin::on_begin_pass(&mut self, ctx: &Context)` — called at the start of every pass; can add windows/panels
- `Plugin::on_end_pass(&mut self, ctx: &Context)` — called at the end of every pass
- `Plugin::input_hook(&mut self, input: &mut RawInput)` — intercept and modify raw input each frame
- `Plugin::output_hook(&mut self, output: &mut FullOutput)` — inspect or modify frame output
- `Plugin::on_widget_under_pointer(&mut self, ctx: &Context, widget: Option<&WidgetInfo>)` — called with the widget under the pointer (0.33.2+; useful for widget inspectors)

### Misc
- `ctx.set_cursor_icon(icon)` — change the mouse cursor
- `ctx.open_url(url)` — open a URL in the browser
- `ctx.text_edit_focused()` — `Option<Id>` of the currently focused text edit widget
- `ctx.os()` / `ctx.set_os(os)` — get/set the reported OS
- `ctx.options(|o| …)` / `ctx.options_mut(|o| …)` — read/write `Options`
- `ctx.tessellation_options(|o| …)` / `ctx.tessellation_options_mut(|o| …)` — read/write tessellation settings
- `ctx.format_shortcut(shortcut)` — format a keyboard shortcut for display
- `ctx.format_modifiers(modifiers)` — format modifier keys for display
- `ctx.check_for_id_clash(id, rect, name)` — warn on duplicate widget IDs
- `ctx.globally_used_rect(rect)` — mark a rect as used globally
- `ctx.send_cmd(cmd)` — send an internal egui command
- `ctx.set_immediate_viewport_renderer(renderer)` — set a custom renderer for immediate viewports
- `ctx.set_request_repaint_callback(cb)` — set a callback invoked when a repaint is requested (useful in custom integrations)

### GUI Zoom helpers (`egui::gui_zoom`)
- `egui::gui_zoom::zoom_with_keyboard(ctx)` — handle Cmd+Plus/Minus/0 to zoom the whole UI (call every frame)
- `egui::gui_zoom::zoom_in(ctx)` / `zoom_out(ctx)` / `zoom_reset(ctx)` — programmatic zoom

---

## egui — Ui

`Ui` is the main building block. All widget calls go through `&mut Ui`.

### Widgets — Basic
- `ui.label(text)` — static text label
- `ui.colored_label(color, text)` — label with explicit colour
- `ui.heading(text)` — large heading text
- `ui.monospace(text)` — monospace text
- `ui.code(text)` — inline code (monospace + background)
- `ui.small(text)` — small text
- `ui.strong(text)` — bold text
- `ui.weak(text)` — dimmed text
- `ui.hyperlink(url)` — clickable link (displays URL)
- `ui.hyperlink_to(label, url)` — clickable link with custom label
- `ui.link(text)` — inline text link (returns Response)
- `ui.separator()` — horizontal/vertical dividing line

### Widgets — Input
- `ui.button(text)` — clickable button
- `ui.small_button(text)` — compact button
- `ui.checkbox(&mut bool, label)` — checkbox
- `ui.radio(selected: bool, label)` — single radio button
- `ui.radio_value(&mut val, variant, label)` — radio for enum/value
- `ui.selectable_label(selected: bool, label)` — toggleable label
- `ui.selectable_value(&mut val, variant, label)` — selectable for enum/value
- `ui.toggle_value(&mut bool, label)` — toggle button (stateful)
- `ui.text_edit_singleline(&mut String)` — single-line text input
- `ui.text_edit_multiline(&mut String)` — multi-line text input
- `ui.code_editor(&mut String)` — code editor (monospace multiline)
- `ui.spinner()` — animated loading spinner
- `ui.image(source)` — display an image
- `ui.drag_angle(&mut f32)` — drag to change an angle (radians)
- `ui.drag_angle_tau(&mut f32)` — drag to change an angle (turns)

### Widgets — Added via `ui.add()`
- `ui.add(Button::new(text))` — full-featured button widget; accepts `IntoAtoms` (text, image, or tuples). Builder options include `.left_text(text)` (left-align text, useful for icon+text menu items), `.shortcut_text(text)`, `.image(img)`, `.fill(color)`, `.stroke(stroke)`, `.min_size(s)`, `.frame(bool)`, `.frame_when_inactive(bool)`, `.sense(sense)`, `.wrap_mode(mode)`
- `ui.add(Slider::new(&mut val, range))` — horizontal slider
- `ui.add(DragValue::new(&mut val))` — drag-to-change numeric input
- `ui.add(TextEdit::singleline(&mut str))` — rich text edit widget
- `ui.add(TextEdit::multiline(&mut str))` — multi-line text edit widget
- `ui.add(Image::new(source))` — image with builder options
- `ui.add(ProgressBar::new(fraction))` — progress bar (0.0–1.0)
- `ui.add(Separator::default())` — separator widget
- `ui.add(Label::new(text))` — label widget with builder options
- `ui.add_sized(size, widget)` — add widget at a fixed size
- `ui.add_enabled(enabled: bool, widget)` — conditionally enable a widget
- `ui.add_visible(visible: bool, widget)` — conditionally show a widget

### Atom Layout (0.32+)
`Atom` is a struct (wrapping `WidgetText`, `Image`, or a custom-size placeholder) used as a low-level layout building block inside widgets. `AtomLayout` handles intra-widget layout. These enable composing widgets from multiple text/image atoms in any order.
- `Atom::text(text)` — text atom
- `Atom::image(image)` — image atom
- `Atom::custom(id, size)` — custom-painted atom (painter-driven; lets you embed arbitrary widget content inside a button, etc.)
- `Atom::grow()` — empty spacer that expands to push preceding atoms left and following atoms right
- `AtomLayout::new(atoms)` — create a layout from atoms (implements `IntoAtoms`)
- `AtomLayout::show(ui, sense, paint_fn)` — allocate, sense, paint; returns `AtomLayoutResponse`
- `AllocatedAtomLayout::paint(painter, ...)` — paint a pre-allocated atom layout
- `Button::new((atom1, atom2, …))` — buttons accept any `IntoAtoms` (0.34+)
- `Button::atom_ui(ui)` — like `Button::ui` but returns `AtomLayoutResponse` with per-atom `Rect`s (needed when using `Atom::custom`)
- `AtomLayoutResponse::rect(atom_id)` — get the `Rect` for a specific atom id
- `DragValue::prefix(atom)` / `.suffix(atom)` — atom prefix/suffix on `DragValue` (0.34+)
- `TextEdit::prefix(atom)` / `.suffix(atom)` — atom prefix/suffix on `TextEdit` (0.34+)

#### `trait AtomExt`
Convenience trait implemented on everything that is `Into<Atom>`:
- `.atom_grow(bool)` — mark atom to expand / fill remaining space
- `.atom_size(Vec2)` — force a fixed size
- `.atom_truncate(bool)` — allow text truncation

### Color Pickers
- `ui.color_edit_button_srgba(&mut Color32)` — RGBA color picker button
- `ui.color_edit_button_srgb(&mut [u8;3])` — RGB color picker button
- `ui.color_edit_button_hsva(&mut Hsva)` — HSVA color picker button
- `ui.color_edit_button_rgba_unmultiplied(&mut [f32;4])` — f32 RGBA picker
- `ui.color_edit_button_rgba_premultiplied(&mut [f32;4])` — premultiplied f32 picker
- `ui.color_edit_button_srgba_unmultiplied(&mut [u8;4])` — u8 RGBA picker
- `ui.color_edit_button_srgba_premultiplied(&mut [u8;4])` — premultiplied u8 picker

### Layout
- `ui.horizontal(|ui| …)` — lay out children left-to-right
- `ui.horizontal_top(|ui| …)` — horizontal, top-aligned
- `ui.horizontal_centered(|ui| …)` — horizontal, vertically centred
- `ui.horizontal_wrapped(|ui| …)` — horizontal with line wrapping
- `ui.vertical(|ui| …)` — lay out children top-to-bottom
- `ui.vertical_centered(|ui| …)` — vertical, horizontally centred
- `ui.vertical_centered_justified(|ui| …)` — vertical, centred + full-width
- `ui.centered_and_justified(|ui| …)` — single widget centred and justified
- `ui.with_layout(Layout, |ui| …)` — arbitrary `Layout` configuration
- `ui.columns(n, |cols| …)` — split into N equal columns
- `ui.columns_const::<N>(|cols| …)` — const-generic column split
- `ui.indent(id, |ui| …)` — indented sub-UI
- `ui.scope(|ui| …)` — temporary style/settings scope
- `ui.scope_builder(UiBuilder, |ui| …)` — scope with a custom `UiBuilder`
- `ui.scope_dyn(UiBuilder, Box<dyn FnOnce(&mut Ui)>)` — scope with a dynamic closure
- `ui.group(|ui| …)` — widgets inside a framed group box
- `ui.push_id(id_source, |ui| …)` — push an id scope

### Containers
- `ui.collapsing(heading, |ui| …)` — collapsible section
- `ui.menu_button(label, |ui| …)` — drop-down menu button
- `ui.menu_image_button(image, |ui| …)` — drop-down menu from an image button
- `ui.menu_image_text_button(image, label, |ui| …)` — image+text drop-down
- `ui.close_menu()` — close the currently open menu
- `ui.close()` / `ui.close_kind(kind)` — close the containing window/area
- `ui.dnd_drag_source(id, payload, |ui| …)` — drag-and-drop source
- `ui.dnd_drop_zone(frame, |ui| …)` — drag-and-drop drop target

### Scroll
- `ui.scroll_to_cursor(align)` — scroll parent `ScrollArea` to show cursor
- `ui.scroll_to_rect(rect, align)` — scroll to show a rect
- `ui.scroll_to_cursor_animation(align, anim)` — animated scroll to cursor
- `ui.scroll_to_rect_animation(rect, align, anim)` — animated scroll to rect
- `ui.scroll_with_delta(delta)` — programmatically scroll by delta
- `ui.scroll_with_delta_animation(delta, anim)` — animated scroll by delta

### Size & Space
- `ui.add_space(px)` — add blank space
- `ui.allocate_space(size)` — reserve a region, returns `(Id, Rect)`
- `ui.allocate_rect(rect, sense)` — allocate a specific `Rect`
- `ui.allocate_at_least(size, sense)` — allocate at least this size
- `ui.allocate_exact_size(size, sense)` — allocate exactly this size
- `ui.allocate_ui(size, |ui| …)` — child ui of a given size
- `ui.allocate_ui_at_rect(rect, |ui| …)` — child ui placed at a rect
- `ui.allocate_ui_with_layout(size, layout, |ui| …)` — child ui with layout
- `ui.allocate_new_ui(UiBuilder, |ui| …)` — fully custom child ui
- `ui.allocate_painter(size, sense)` — raw `Painter` for custom drawing
- `ui.allocate_response(size, sense)` — allocate space and return a Response
- `ui.put(rect, widget)` — place a widget at an exact `Rect` (advances cursor)
- `ui.place(atom_layout, |ui| …)` — place using atom layout **without** advancing the cursor (0.32.2+; useful for badge/overlay widgets)
- `ui.available_size()` — remaining space in the current direction
- `ui.available_width()` / `ui.available_height()` — remaining width/height
- `ui.available_size_before_wrap()` — space before wrapping
- `ui.available_rect_before_wrap()` — rect before wrapping
- `ui.min_rect()` — smallest rect enclosing all widgets so far
- `ui.max_rect()` — maximum available rect
- `ui.min_size()` — minimum size needed
- `ui.set_min_width(w)` / `ui.set_min_height(h)` / `ui.set_min_size(s)` — set min dimensions
- `ui.set_max_width(w)` / `ui.set_max_height(h)` / `ui.set_max_size(s)` — set max dimensions
- `ui.set_width(w)` / `ui.set_height(h)` — set exact dimensions
- `ui.set_width_range(range)` / `ui.set_height_range(range)` — constrain a dimension
- `ui.set_row_height(h)` — fix the current row height
- `ui.shrink_width_to_current()` / `ui.shrink_height_to_current()` — shrink to used area
- `ui.take_available_width()` / `ui.take_available_height()` / `ui.take_available_space()` — consume remaining space (sets min size to available)
- `ui.expand_to_include_rect(rect)` — expand min rect to include a rect
- `ui.expand_to_include_x(x)` / `ui.expand_to_include_y(y)` — expand to include a coordinate
- `ui.advance_cursor_after_rect(rect)` — move the layout cursor past a rect

### Interaction
- `ui.interact(rect, id, sense)` — create a `Response` for a region
- `ui.interact_bg(sense)` — interact with the full ui background
- `ui.interact_opt(rect, id, sense)` — interact, returns `Option<Response>`
- `ui.interact_with_hovered(rect, hovered, id, sense)` — interact with known hover state
- `ui.rect_contains_pointer(rect)` — check if pointer is in rect
- `ui.ui_contains_pointer()` — pointer is anywhere inside this ui
- `ui.response()` — current ui's response

### Cursor & Position
- `ui.cursor()` — current layout cursor rect
- `ui.next_widget_position()` — position of the next widget
- `ui.next_auto_id()` — peek at the next auto-generated id
- `ui.painter()` — `Painter` for the full ui clip rect
- `ui.painter_at(rect)` — `Painter` clipped to `rect`
- `ui.with_layer_id(layer_id, |ui| …)` — draw on a specific layer
- `ui.debug_paint_cursor()` — visualise the current cursor (debug)

### Id Management
- `ui.id()` — this ui's `Id`
- `ui.auto_id_with(suffix)` — generate a child id with suffix
- `ui.make_persistent_id(source)` — create a stable persistent id
- `ui.push_stack_info(info, |ui| …)` — push `UiStackInfo` onto the stack
- `ui.skip_ahead_auto_ids(n)` — skip N auto ids to keep ids stable
- `ui.unique_id()` — allocate a guaranteed-unique id

### Visibility & Enable State
- `ui.is_enabled()` — whether the ui accepts input
- `ui.set_enabled(bool)` — enable/disable all child widgets
- `ui.disable()` — shorthand to disable
- `ui.is_visible()` — whether the ui will paint
- `ui.set_visible(bool)` — show/hide without affecting layout
- `ui.set_invisible()` — shorthand to hide
- `ui.is_rect_visible(rect)` — whether a given rect is within the clip rect (skip expensive work)
- `ui.is_tooltip()` — whether this ui lives inside a tooltip
- `ui.add_enabled_ui(bool, |ui| …)` — conditional enabled scope
- `ui.add_visible_ui(bool, |ui| …)` — conditional visible scope
- `ui.is_sizing_pass()` / `ui.set_sizing_pass(bool)` — multi-pass sizing state
- `ui.opacity()` / `ui.set_opacity(f)` / `ui.multiply_opacity(f)` — alpha control

### Style Access
- `ui.style()` — read current `Style`
- `ui.style_mut()` — mutate the local style (affects this ui only)
- `ui.reset_style()` — revert to the inherited style
- `ui.visuals()` — read current `Visuals`
- `ui.visuals_mut()` — mutate local `Visuals`
- `ui.spacing()` — read `Spacing`
- `ui.spacing_mut()` — mutate local `Spacing`
- `ui.set_style(style)` — replace local style
- `ui.wrap_mode()` — current text wrap mode
- `ui.wrap_text()` — whether text wrapping is enabled (deprecated helper)
- `ui.text_valign()` — vertical text alignment
- `ui.text_style_height(style)` — pixel height of a `TextStyle`
- `ui.pixels_per_point()` — current DPI scale

### Stack & Layers
- `ui.stack()` — access the `UiStack` (parent hierarchy)
- `ui.layer_id()` — current rendering layer
- `ui.clip_rect()` — current clip rectangle
- `ui.set_clip_rect(rect)` — override clip rectangle
- `ui.shrink_clip_rect(rect)` — intersect clip rect with rect
- `ui.layout()` — current `Layout`
- `ui.ctx()` — the underlying `Context`

### Grid
- `ui.end_row()` — move to the next row in a `Grid`

### Window / Area close
- `ui.should_close()` — returns true if the enclosing window was closed
- `ui.will_parent_close()` — parent container is about to close
- `ui.with_visual_transform(transform, |ui| …)` — apply a visual transform

### Child UIs (low-level)
- `ui.child_ui(rect, layout, options)` — create a free-form child ui
- `ui.child_ui_with_id_source(rect, layout, id, options)` — child ui with explicit id source
- `ui.new_child(UiBuilder)` — create a child from a `UiBuilder`

---

## Containers (standalone)

### `Window`
- `Window::new(title).show(ctx, |ui| …)` — floating, draggable window
- `Window::from_viewport(id, ViewportBuilder)` — window following a viewport's settings
- `.id(id)` — override the id (required if title changes)
- `.open(&mut bool)` — add a close button and track open state
- `.enabled(bool)` — gray out and disable if false
- `.interactable(bool)` — if false, clicks pass through
- `.movable(bool)` — allow/disallow dragging
- `.order(Order)` — layer order (`Order::Foreground` keeps on top)
- `.fade_in(bool)` / `.fade_out(bool)` — animate appearance/disappearance
- `.frame(Frame)` — override background, margins, stroke
- `.title_bar(bool)` — show or hide the title bar
- `.collapsible(bool)` — allow minimising to title bar
- `.resizable(bool)` / `.resize(|r| …)` — resize behaviour
- `.auto_sized()` — fit window to content each frame
- `.default_pos(pos)` / `.current_pos(pos)` / `.fixed_pos(pos)` — position
- `.default_size(size)` / `.fixed_size(size)` / `.default_rect(rect)` / `.fixed_rect(rect)`
- `.default_width(w)` / `.default_height(h)`
- `.min_size(s)` / `.min_width(w)` / `.min_height(h)`
- `.max_size(s)` / `.max_width(w)` / `.max_height(h)`
- `.anchor(Align2, offset)` — pin to a corner of the screen
- `.pivot(Align2)` — which point of the window is anchored
- `.constrain(bool)` / `.constrain_to(rect)` — keep inside screen
- `.scroll(Vec2b)` — enable scroll axes; shorthand `.vscroll(bool)` / `.hscroll(bool)`
- `.drag_to_scroll(bool)` — drag contents to scroll
- `.scroll_bar_visibility(v)` — control scrollbar display
- `.mutate(|w| …)` — arbitrary builder mutation

### Panels
> **0.34 Note:** `SidePanel` and `TopBottomPanel` are **deprecated** — replaced by the unified `Panel`. `SidePanel` is now a type alias for `Panel`. Using panels directly on `Context` is also deprecated; use `show_inside` with `Ui` instead. `CentralPanel::show(ctx, …)` is deprecated; prefer `show_inside(ui, …)`.

**`CentralPanel`** — covers all remaining space (must be added last)
- `CentralPanel::default().show_inside(ui, |ui| …)` — fills remaining space inside a `Ui` (**preferred**)
- `CentralPanel::default().show(ctx, |ui| …)` — *(deprecated 0.34)* top-level show
- `CentralPanel::no_frame()` — no margin or background
- `CentralPanel::default_margins()` — background + inner margins
- `.frame(Frame)` — override background, margins, stroke

**`Panel`** — covers one full side of a `Ui` or the screen (unified replacement for `SidePanel` + `TopBottomPanel`)
- `Panel::left(id).show_inside(ui, |ui| …)` — left side panel (**preferred**)
- `Panel::right(id).show_inside(ui, |ui| …)` — right side panel
- `Panel::top(id).show_inside(ui, |ui| …)` — top panel (not resizable by default)
- `Panel::bottom(id).show_inside(ui, |ui| …)` — bottom panel (not resizable by default)
- `Panel::left(id).show(ctx, |ui| …)` — *(deprecated 0.34)* top-level show; prefer `show_inside` inside `CentralPanel`
- `Panel::show_animated(ctx, visible, |ui| …)` — fade in/out based on bool
- `Panel::show_animated_inside(ui, visible, |ui| …)` — animated inside a Ui
- `Panel::show_animated_between(ctx, show_a, left_panel, right_panel)` — animate between two panels
- `.resizable(bool)` — allow drag-resize (default `true` for left/right, `false` for top/bottom)
- `.show_separator_line(bool)` — show the separator even when not hovered
- `.default_size(f)` / `.min_size(f)` / `.max_size(f)` / `.exact_size(f)` / `.size_range(range)`
- `.default_width(f)` / `.min_width(f)` / `.max_width(f)` / `.exact_width(f)` / `.width_range(range)`
- `.default_height(f)` / `.min_height(f)` / `.max_height(f)` / `.exact_height(f)` / `.height_range(range)`
- `.frame(Frame)` — override background, margins, stroke

### `ScrollArea`
- `ScrollArea::vertical().show(ui, |ui| …)` — vertical scroll
- `ScrollArea::horizontal().show(ui, |ui| …)` — horizontal scroll
- `ScrollArea::both().show(ui, |ui| …)` — bidirectional scroll
- `ScrollArea::neither().show(ui, |ui| …)` — no scrolling (clip only)
- `ScrollArea::new(Vec2b)` — custom per-axis scroll enable
- `.id_salt(source)` — disambiguate multiple scroll areas
- `.max_height(h)` / `.max_width(w)` — constrain outer size
- `.min_scrolled_height(h)` / `.min_scrolled_width(w)` — minimum scrollable dimension
- `.auto_shrink(Vec2b)` — shrink to fit content on each axis
- `.scroll_bar_visibility(ScrollBarVisibility)` — always/never/when needed
- `.scroll_bar_rect(rect)` — restrict scrollbar to a sub-rect (e.g. below sticky header)
- `.scroll_offset(Vec2)` — set initial scroll offset
- `.vertical_scroll_offset(f)` / `.horizontal_scroll_offset(f)` — per-axis offset
- `.drag_to_scroll(bool)` — enable drag-to-scroll (touch-friendly)
- `.enable_scrolling(bool)` — toggle scrolling at runtime
- `.scroll(Vec2b)` / `.vscroll(bool)` / `.hscroll(bool)` — enable per-axis scrolling
- `.scroll_source(ScrollSource)` — control what triggers scrolling
- `.stick_to_bottom(bool)` / `.stick_to_right(bool)` — keep scroll pinned to end
- `.animated(bool)` — animate scroll position changes
- `.wheel_scroll_multiplier(f)` — scale mouse-wheel scroll speed
- `.content_margin(margin)` — margin around inner content
- `.on_hover_cursor(cursor)` / `.on_drag_cursor(cursor)` — cursor icons
- `.fade_edge(bool)` — fade out content near the edges of the scroll area (0.34+, default `true`)
- `.show_rows(ui, row_h, total, |ui, range| …)` — virtualised row rendering
- `.show_viewport(ui, |ui, viewport_rect| …)` — manual viewport callback

### `Area`
- `Area::new(id).show(ctx, |ui| …)` — free-floating area at any position
- `.fixed_pos(pos)`, `.default_pos(pos)`, `.anchor(align, offset)`
- `.movable(bool)`, `.interactable(bool)`, `.order(Order)`
- `.constrain(bool)`, `.constrain_to(rect)`

### `Frame`
- `Frame::new().show(ui, |ui| …)` — draw a frame/border around content
- `Frame::none()`, `Frame::dark_canvas()`, `Frame::canvas()`, `Frame::window(style)`, `Frame::menu(style)`, `Frame::popup(style)`, `Frame::side_top_panel(style)`, `Frame::central_panel(style)`, `Frame::group(style)`
- `.inner_margin(margin)`, `.outer_margin(margin)`, `.corner_radius(r)`, `.stroke(stroke)`, `.fill(color)`, `.shadow(shadow)`

### `Grid`
- `Grid::new(id).show(ui, |ui| …)` — table-like grid layout
- `.num_columns(n)`, `.spacing(vec2)`, `.min_col_width(w)`, `.min_row_height(h)`
- `.max_col_width(w)`, `.striped(bool)`, `.start_row(n)`

### `CollapsingHeader`
- `CollapsingHeader::new(label).show(ui, |ui| …)` — collapsible with header
- `.default_open(bool)`, `.open(Option<bool>)`, `.id_salt(source)`
- `.show_unindented(ui, |ui| …)` — no indentation

### `ComboBox`
- `ComboBox::from_label(label).selected_text(text).show_ui(ui, |ui| …)` — drop-down combo
- `ComboBox::from_id_salt(id).show_index(ui, &mut idx, len, label_fn)` — index-based combo
- `.width(w)`, `.height(h)`, `.wrap_mode(mode)`, `.truncate(bool)`, `.icon(fn)`

### `Popup`
- `Popup::new(id)` — free-floating popup widget
- `Popup::below_widget(response, id)` — popup anchored below a widget

> **Note:** The old `Memory::popup` open/close API was deprecated in 0.32 in favour of the new `Popup` struct. Use `Popup::below_widget` for dropdown-style popups and `Modal` for modal overlays.

### `Modal`
- `Modal::new(id).show(ctx, |ui| …)` — modal dialog overlay
- `.backdrop_color(color)`, `.frame(frame)`

### `Resize`
- `Resize::default().show(ui, |ui| …)` — user-resizable container
- `.default_size(size)`, `.min_size(s)`, `.max_size(s)`, `.resizable(bool)`

### `Scene`
- `Scene::new().show(ui, |ui| …)` — pannable/zoomable 2D canvas
- `.zoom_range(range)`, `.max_inner_size(size)`

### `Sides`
- `Sides::new().show(ui, |left_ui| …, |right_ui| …)` — two-sided layout

### `Tooltip`
- `Tooltip::new(id)` — manual tooltip placement
- `ui.response().on_hover_text(text)` — auto tooltip on hover
- `ui.response().on_hover_ui(|ui| …)` — custom tooltip UI on hover
- `ui.response().on_disabled_hover_text(text)` — tooltip while disabled

### `MenuBar`
- `MenuBar::new().ui(ui, |ui| …)` — horizontal menu bar

---

## Painter (Low-level Drawing)

- `Painter::new(ctx, layer_id, clip_rect)` — create a painter on a layer with clip rect
- `painter.with_layer_id(layer_id)` — redirect to a different layer (returns new painter)
- `painter.with_clip_rect(rect)` — sub-painter clipped to intersection of rects
- `painter.set_layer_id(layer_id)` — mutably redirect layer
- `painter.set_clip_rect(rect)` — replace clip rect
- `painter.shrink_clip_rect(rect)` — intersect clip rect
- `painter.clip_rect()` — current clip rect
- `painter.layer_id()` — current layer id
- `painter.ctx()` — parent `Context`
- `painter.pixels_per_point()` — DPI scale factor
- `painter.fonts(|f| …)` / `painter.fonts_mut(|f| …)` — access font system
- `painter.is_visible()` — false if painter is invisible or pass will be discarded
- `painter.set_invisible()` — suppress all output from this painter
- `painter.opacity()` / `painter.set_opacity(f)` / `painter.multiply_opacity(f)` — alpha control
- `painter.add(shape)` — add a `Shape`, returns `ShapeIdx`
- `painter.extend(shapes)` — add multiple `Shape`s
- `painter.set(shape_idx, shape)` — overwrite a previously reserved shape
- `painter.for_each_shape(|shape| …)` — iterate over all shapes added this frame
- `painter.rect(rect, corner_radius, fill, stroke)` — filled + stroked rectangle
- `painter.rect_filled(rect, corner_radius, fill)` — filled rectangle
- `painter.rect_stroke(rect, corner_radius, stroke)` — stroked rectangle
- `painter.circle(center, radius, fill, stroke)` — full circle
- `painter.circle_filled(center, radius, fill)` — filled circle
- `painter.circle_stroke(center, radius, stroke)` — stroked circle
- `painter.line_segment([p1, p2], stroke)` — single line segment
- `painter.line(points, stroke)` — polyline through points
- `painter.hline(x_range, y, stroke)` — horizontal line
- `painter.vline(x, y_range, stroke)` — vertical line
- `painter.arrow(origin, vec, stroke)` — arrow with head
- `painter.image(texture_id, rect, uv, tint)` — paint a texture
- `painter.text(pos, anchor, text, font_id, color)` — lay out and paint text
- `painter.galley(pos, galley, color)` — paint a pre-laid `Galley`
- `painter.galley_with_override_text_color(pos, galley, color)` — galley with colour override
- `painter.layout(text, font_id, color, wrap_width)` — lay out text into a `Galley`
- `painter.layout_no_wrap(text, font_id, color)` — lay out text without wrapping
- `painter.layout_job(job)` — lay out a `LayoutJob` into a `Galley`
- `painter.debug_rect(rect, color, text)` — paint a debug rect with label
- `painter.debug_text(pos, anchor, color, text)` — paint debug text
- `painter.error(pos, text)` — paint a debug error marker
- `painter.round_to_pixel(f)` / `painter.round_to_pixel_center(f)` — snap scalar to pixel
- `painter.round_pos_to_pixels(pos)` / `painter.round_pos_to_pixel_center(pos)` — snap position
- `painter.round_vec_to_pixels(vec)` — snap vector to pixel grid
- `painter.round_rect_to_pixels(rect)` — snap rect to pixel grid

---

## egui_extras — Official Extension Crate

`egui_extras` provides common higher-level widgets not in the core. Add `egui_extras = "0.34"` to your `Cargo.toml`.

### Image Loaders (`egui_extras::install_image_loaders`)
Call `egui_extras::install_image_loaders(ctx)` at startup (typically in `CreationContext`) to enable loading images from URLs, local paths, and embedded bytes.
- Feature `all_loaders` enables all built-in loaders (requires the `image` crate with appropriate format features).
- Individual loader features: `file`, `http`, `svg` (via `resvg`), `gif`.

### `TableBuilder` (feature `table`)
- `TableBuilder::new(ui)` — create a table builder
- `.column(Column::auto())` / `.column(Column::exact(w))` / `.column(Column::remainder())` / `.column(Column::initial(w))` — define columns
- `.resizable(bool)` — make columns draggable
- `.striped(bool)` — alternate row background
- `.header(row_height, |header| …)` — render the header row
- `.body(|body| …)` — render body rows
  - `body.row(row_height, |row| …)` — non-virtualised row
  - `body.rows(row_height, total_rows, |row_idx, row| …)` — virtualised rows
  - `row.col(|ui| …)` — render a cell

### `RetainedImage` (deprecated since 0.23)
Use `egui::Image` with the loader system instead.

### `DatePickerButton` (feature `datepicker`)
- `DatePickerButton::new(&mut NaiveDate).ui(ui)` — calendar popup date picker (requires `chrono`)

### `Syntax Highlighting` (feature `syntect`)
- `egui_extras::syntax_highlighting::code_view_ui(ui, theme, code, lang)` — show syntax-highlighted code

---

## egui_kittest — UI Testing

`egui_kittest` is the official snapshot/interaction testing crate for egui UIs.

- `Harness::new_ui(|ui| …)` — create a test harness with a `Ui` callback
- `Harness::new_ui_state(state, |ui, state| …)` — harness with mutable state
- `Harness::run()` — advance one frame
- `Harness::run_steps(n)` — advance multiple frames
- `Harness::mask(rect)` — mask a `Rect` in snapshot images with a bright color (0.32.2+; useful for hiding dynamic/unstable UI regions)
- `harness.get_by_label(text)` — find a node by accessible label
- `harness.get_by_role(role)` — find a node by accessibility role
- `harness.get_by_id(id)` — find by egui `Id`
- `node.click()` — simulate a click
- `node.drag_to(other)` — drag from one node to another (0.33.3+)
- `node.drop_on(other)` — drag-and-drop helper (0.33.3+)
- `node.type_text(text)` — type text into a focused text widget
- `harness.snapshot(name)` — assert pixel-identical snapshot (requires `wgpu` feature)
- `harness.snapshot_options(name, opts)` — snapshot with comparison options (threshold, etc.)

> **kitdiff** — a companion web/CLI tool ([rerun-io.github.io/kitdiff](https://rerun-io.github.io/kitdiff/)) for diffing snapshot images in CI. Install via `cargo install --git https://github.com/rerun-io/kitdiff`.

---

## Response

Returned by every widget call. Public fields: `.ctx`, `.id`, `.rect`, `.interact_rect`, `.layer_id`, `.sense`.

### Clicks & Pointer
- `.clicked()` — primary click this frame
- `.clicked_by(PointerButton)` — click by a specific button
- `.secondary_clicked()` — right-click this frame
- `.middle_clicked()` — middle-click this frame
- `.double_clicked()` / `.double_clicked_by(button)` — double click
- `.triple_clicked()` / `.triple_clicked_by(button)` — triple click
- `.clicked_elsewhere()` — primary click occurred outside this widget
- `.clicked_with_open_in_background(modifiers)` — click with modifier to open in background
- `.long_touched()` — long press on touch screen
- `.hovered()` — pointer hovering this widget (false if disabled)
- `.contains_pointer()` — pointer inside rect (true even when dragging something else)
- `.hover_pos()` — pointer position if hovering
- `.interact_pointer_pos()` — pointer position during click or drag
- `.is_pointer_button_down_on()` — a pointer button is currently pressed on this widget

### Drag
- `.dragged()` / `.dragged_by(button)` — widget is being dragged
- `.drag_started()` / `.drag_started_by(button)` — drag began this frame
- `.drag_stopped()` / `.drag_stopped_by(button)` — drag released this frame
- `.drag_delta()` — pointer delta this frame during drag
- `.drag_motion()` — pointer delta this frame regardless of drag state
- `.total_drag_delta()` — total delta accumulated over the whole drag

### Drag-and-Drop
- `.dnd_set_drag_payload::<T>(payload)` — set a typed payload when dragging starts
- `.dnd_hover_payload::<T>()` — get payload if something of type T is being dragged over
- `.dnd_release_payload::<T>()` — get payload if something of type T was dropped here

### State Changes
- `.changed()` — value was changed this frame (inputs, checkboxes, sliders, etc.)
- `.mark_changed()` — manually mark as changed for custom widgets
- `.enabled()` — whether the widget is enabled

### Focus
- `.gained_focus()` / `.lost_focus()` / `.has_focus()` — keyboard focus state
- `.request_focus()` — request keyboard focus
- `.surrender_focus()` — release keyboard focus

### Scrolling
- `.scroll_to_me(align)` — scroll parent `ScrollArea` to show this widget
- `.scroll_to_me_animation(align, animation)` — animated scroll to this widget

### Tooltips & Cursor
- `.on_hover_text(text)` — show tooltip text on hover
- `.on_hover_text_at_pointer(text)` — tooltip following the pointer
- `.on_hover_ui(|ui| …)` — show custom tooltip UI on hover
- `.on_hover_ui_at_pointer(|ui| …)` — custom tooltip following the pointer
- `.on_disabled_hover_text(text)` — tooltip shown while disabled
- `.on_disabled_hover_ui(|ui| …)` — custom tooltip while disabled
- `.on_hover_cursor(cursor)` — change cursor icon on hover
- `.on_hover_and_drag_cursor(cursor)` — change cursor on hover and during drag
- `.show_tooltip_text(text)` — unconditionally show tooltip text
- `.show_tooltip_ui(|ui| …)` — unconditionally show tooltip UI
- `.is_tooltip_open()` — whether a tooltip is currently visible

### Context Menu
- `.context_menu(|ui| …)` — add a right-click context menu
- `.context_menu_opened()` — whether the context menu is open

### Accessibility & Misc
- `.labelled_by(id)` — associate a label with this widget for accessibility
- `.widget_info(info)` — provide `WidgetInfo` for accessibility
- `.widget_state()` — read `WidgetState` for this widget
- `.output_event(event)` — emit an accessibility event
- `.paint_debug_info()` — paint debug overlay on this widget
- `.highlight()` — visually highlight this widget this frame
- `.interact(sense)` — add additional sensing to an existing response
- `.union(other)` — logical OR of two responses (`.hovered()` etc. combined)
- `.with_new_rect(rect)` — clone response with a new rect
- `.set_close()` / `.should_close()` — signal/query that the container should close
- `.parent_id()` — id of the parent `Ui`
- `.intrinsic_size()` / `.set_intrinsic_size(size)` — natural size hint for layout crates

---

## InputState

Accessed via `ctx.input(|i| …)`.

- `i.key_pressed(Key)` / `i.key_down(Key)` / `i.key_released(Key)` — keyboard
- `i.key_pressed_with_modifiers(key, modifiers)` — key press with modifier check
- `i.modifiers` — `Modifiers` (alt, ctrl, shift, mac_cmd, command)
- `i.consume_key(modifiers, key)` — consume a key event (marks it as handled)
- `i.consume_shortcut(shortcut)` — consume a `KeyboardShortcut`
- `i.match_shortcut(shortcut, key_state)` — check a shortcut without consuming
- `i.pointer` — `PointerState` (position, buttons, delta, etc.)
  - `pointer.hover_pos()` — current hover position (if pointer is in window)
  - `pointer.interact_pos()` — position during a click or drag
  - `pointer.button_down(btn)` / `.button_pressed(btn)` / `.button_released(btn)`
  - `pointer.primary_down()` / `.primary_pressed()` / `.primary_released()`
  - `pointer.secondary_down()` / `.secondary_pressed()` / `.secondary_released()`
  - `pointer.delta()` — pointer movement this frame
  - `pointer.velocity()` — recent average pointer velocity
  - `pointer.total_drag_delta()` — accumulated drag delta since drag started (0.33.2+)
  - `pointer.any_down()` / `pointer.any_pressed()` / `pointer.any_released()` / `pointer.any_click()`
- `i.scroll_delta` — scroll wheel delta (points)
- `i.smooth_scroll_delta` — smoothed scroll delta
- `i.zoom_delta()` — pinch-to-zoom scale factor
- `i.zoom_delta_2d()` — 2D zoom delta (separate x/y)
- `i.rotation_delta()` — rotation gesture delta in radians (trackpad rotation, 0.33+)
- `i.screen_rect()` — current screen rect
- `i.time` — current time (seconds since start)
- `i.unstable_dt` — raw delta time (may spike)
- `i.stable_dt` — stable delta time for animations
- `i.predicted_dt` — predicted next frame dt
- `i.max_texture_side` — GPU max texture dimension
- `i.events` — raw `Vec<Event>` this frame
- `i.raw` — the full `RawInput`
- `i.viewport()` — `&ViewportInfo` for the current viewport (size, focus, occluded, etc.)
- `i.focused` — whether the app has keyboard focus

---

## Structs & Types (key)

| Type | Purpose |
|---|---|
| `Color32` | 8-bit RGBA colour (`Color32::RED`, `::from_rgb(r,g,b)`, etc.) |
| `Rgba` | f32 linear RGBA |
| `Vec2` | 2D vector (`Vec2::new(x,y)`, `vec2(x,y)`) |
| `Pos2` | 2D position (`Pos2::new(x,y)`, `pos2(x,y)`) |
| `Rect` | Axis-aligned rect (`Rect::from_min_max`, `::from_center_size`, etc.) |
| `Rangef` | `f32` range (`Rangef::new(min,max)`) |
| `Margin` | Per-side margin (`Margin::same(f)`, `::symmetric(x,y)`) |
| `Stroke` | Line stroke (`Stroke::new(width, color)`, `Stroke::NONE`) |
| `Shadow` | Box shadow (`Shadow::small_dark()`, etc.) |
| `CornerRadius` | Corner radii (`CornerRadius::same(f)`) |
| `FontId` | Font + size (`FontId::proportional(f)`, `::monospace(f)`) |
| `TextFormat` | Rich-text formatting (color, font, underline, etc.) |
| `Id` | Stable widget ID (`Id::new(source)`, `Id::NULL`) |
| `Layout` | Describes flow direction and alignment |
| `Sense` | What a widget responds to (`Sense::click()`, `::drag()`, `::hover()`, `::click_and_drag()`) |
| `Align` | `Min`, `Center`, `Max` |
| `Align2` | 2D alignment corner pair |
| `TextureHandle` | Owned GPU texture |
| `TextureOptions` | Filtering, wrap mode for textures |
| `RawInput` | Platform input for one frame |
| `FullOutput` | Egui output for one frame (shapes, platform output, textures) |
| `ViewportBuilder` | Configure a viewport/window (size, title, icon, decorations, etc.) |
| `RichText` | Text with optional per-run style — use `.size(f)`, `.color(c)`, `.font(FontId)`, `.strong()`, `.weak()`, `.italics()`, `.underline()`, `.strikethrough()`, `.monospace()`, `.heading()`, `.small()`, `.extra_letter_spacing(f)`, `.line_height(f)` |
| `Atom` | Low-level widget building block wrapping text, image, or a custom-size placeholder |
| `Atoms` | A list of `Atom`s (implements `IntoAtoms`) |
| `AtomLayout` | Intra-widget layout utility for composing `Atom`s |
| `AtomLayoutResponse` | Response from `AtomLayout::show` (includes per-atom rects via `.rect(atom_id)`) |
| `AllocatedAtomLayout` | Pre-allocated atom layout ready for painting |
| `DragAndDrop` | Context plugin tracking drag-and-drop payload state |
| `SafeAreaInsets` | Screen safe-area insets (iOS notch, system UI, etc.) |
| `KeyboardShortcut` | A keyboard shortcut (`KeyboardShortcut::new(modifiers, key)`) |
| `EventFilter` | Controls which events a focused widget captures exclusively |
| `RepaintCause` | Reason a repaint was requested |
| `RectAlign` | Positions a child rect relative to a parent rect |
| `Options` | Global egui options (`Options::max_passes`, theme, etc.) |
| `InputOptions` | Options for input state handling |
| `InteractOptions` | How to handle multiple `Response::interact` calls |
| `SurrenderFocusOn` | Enum controlling when a widget surrenders keyboard focus (0.33+) |

### Traits (key)

| Trait | Purpose |
|---|---|
| `Plugin` | Trait-based plugin API (0.33+); implement to hook into pass lifecycle, input and output |
| `Widget` | Core widget trait: `fn ui(self, ui: &mut Ui) -> Response` |
| `AtomExt` | Convenience builder methods (`.atom_grow`, `.atom_size`, `.atom_truncate`) on anything `Into<Atom>` |
| `IntoAtoms` | Convert a value (or tuple of values) into `Atoms`; implemented for tuples of `Into<Atom>` |

---

## Enums (key)

- `Key` — keyboard keys (`Key::A`…`Key::Z`, `Key::Enter`, `Key::Escape`, `Key::ArrowUp`, etc.)
- `PointerButton` — `Primary`, `Secondary`, `Middle`, `Extra1`, `Extra2`
- `CursorIcon` — `Default`, `Text`, `Grab`, `Crosshair`, `ResizeHorizontal`, etc.
- `TextStyle` — `Small`, `Body`, `Monospace`, `Button`, `Heading`, `Name(…)`
- `Order` — layer order (`Background`, `Middle`, `Foreground`, `Tooltip`, `Debug`)
- `Shape` — `Rect`, `Circle`, `Ellipse`, `LineSegment`, `Path`, `Text`, `Mesh`, `Callback`, etc.
- `Event` — `Key`, `PointerMoved`, `PointerButton`, `Scroll`, `Text`, `Touch`, `Zoom`, `Rotation` (trackpad rotation, 0.33+), etc.
- `Theme` — `Light`, `Dark`
- `Direction` — `LeftToRight`, `RightToLeft`, `TopDown`, `BottomUp`
- `Align` — `Min`, `Center`, `Max`
- `TextWrapMode` — `Wrap`, `Extend`, `Truncate`
- `SurrenderFocusOn` — `ClickOutside`, `Never`, `PressEscape` — controls when keyboard focus is released (0.33+)

---

## Macros

- `egui::include_image!(path)` — embed a local image at compile time
- `egui::hex_color!("rrggbb")` — parse a hex colour literal at compile time (feature `color-hex`; macro is `const` as of 0.33)
- `egui::github_link_file!(relative_path, label)` — hyperlink to this file on GitHub (demo use)
- `egui::github_link_file_line!(relative_path, label)` — hyperlink with line number
- `egui::generate_loader_id!(type_name)` — generate a unique ID for custom image/bytes/texture loaders

---

## Feature Flags Summary

| Feature | Default | Purpose |
|---|---|---|
| `accesskit` | ✗ | Platform accessibility via AccessKit (enabled by default in eframe, opt-in in egui) |
| `default_fonts` | ✓ | Bundle built-in fonts |
| `glow` (eframe) | ✗ | OpenGL backend via `egui_glow` (was default pre-0.34) |
| `wgpu` (eframe) | ✓ | wgpu backend — **now the default renderer** |
| `wgpu_no_default_features` (eframe) | ✓ | Like `wgpu` but without wgpu's own default backends; lets you pick e.g. `dx12`, `metal`, `webgl` manually |
| `persistence` | ✗ | Save app state to disk |
| `wayland` (eframe) | ✓ | Wayland support on Linux |
| `x11` (eframe) | ✓ | X11 support on Linux |
| `web_screen_reader` | ✓ | Screen reader on web |
| `serde` | ✗ | Serialisation of egui types |
| `rayon` | ✗ | Parallel tessellation |
| `mint` | ✗ | Math library interop (glam, nalgebra, etc.) |
| `color-hex` | ✗ | `hex_color!` macro (the macro is `const` as of 0.33) |
| `bytemuck` | ✗ | Cast `Vertex`, `Vec2`, etc. to `&[u8]` |
| `cint` | ✗ | Interop with other color libraries |
| `unity` | ✗ | Change `Vertex` layout for Unity compatibility |
| `callstack` | ✗ | Show debug-UI with stacktrace on hover (not web) |
| `document-features` | ✗ | Enable when generating docs (optional dep) |

> **Removed features:** `deadlock_detection` (removed 0.33 — detection is always on in debug builds) · `log` (removed 0.33 — `log` crate is always used)

---

## 0.33–0.34 Migration Notes

### 0.34.2 Changes (2026-05-04)
- **`is_pointer_over_egui` bug fixed** — `ctx.is_pointer_over_egui()` had an incorrect result in some cases before 0.34.2; now reliable.
- **Font variation live-update bug fixed** — variable font axis changes now render correctly (was broken in 0.34.0/0.34.1).
- Minor internal optimisations (`Response` struct shrunk slightly).

### 0.34 Changes
- **`App::update` is deprecated** — split your logic into `App::logic` (runs before `ui`, also when UI is hidden) and `App::ui` (rendering only). `App::update` still works but shows deprecation warnings.
- **`Ui` now derefs to `Context`** — all `ui.ctx().foo(…)` calls can be shortened to `ui.foo(…)` directly.
- **`ctx.run_ui` added as preferred entry point** — `ctx.run(input, |ctx| …)` still works but is now considered the legacy form; the new preferred form is `ctx.run_ui(input, |ui| …)` which provides a top-level `Ui`. Custom integrations should migrate.
- **wgpu is now the default renderer** — if you relied on glow being default, add `features = ["glow"]` and set `NativeOptions::renderer = Renderer::Glow`. Add `wgpu = "25"` to your `Cargo.toml` to opt in to wgpu's default backends.
- **`SidePanel` and `TopBottomPanel` deprecated** — replaced by the unified `Panel`. `SidePanel` is kept as a type alias. Migrate: `SidePanel::left("id")` → `Panel::left("id")`; `TopBottomPanel::top("id")` → `Panel::top("id")`. Builder methods `min_width` → `min_size`, `width_range` → `size_range`, etc. (the old names are still present with deprecation warnings).
- **`Panel::show(ctx, …)` and `CentralPanel::show(ctx, …)` deprecated** — use `show_inside(ui, …)` inside a `CentralPanel` callback instead. Panels should no longer be added directly to `Context`.
- **`Context::style()` renamed** — `ctx.style()` and `ctx.style_mut()` are deprecated in favour of `ctx.global_style()` / `ctx.global_style_mut()`, to disambiguate from `ui.style()` / `ui.style_mut()`.
- **`ctx.available_rect()` and `ctx.used_size()` deprecated** — use the `InnerResponse` rect from your panel, or `ctx.used_rect()` respectively.
- **Viewports now pass `&mut Ui`** — the `show_viewport_immediate` / `show_viewport_deferred` callbacks previously received `&Context`; they now receive `&mut Ui`.
- **`screen_rect` vs `content_rect`** — `ctx.screen_rect()` is the full viewport; `ctx.content_rect()` is the safe-area-inset region. Update code to use `content_rect` for placing widgets on iOS.
- **Font rendering changed** — `ab_glyph` replaced by `skrifa` + `vello_cpu`. Text is sharper with hinting and font variations are now supported (live variation changes had a bug in 0.34.0, fixed in 0.34.2). Custom font subpixel positioning may render slightly differently.
- **`CornerRadius` replaces `Rounding`** — renamed in 0.30. If upgrading from pre-0.30 code, rename all `Rounding` → `CornerRadius`.
- **`egui_wants_keyboard_input` / `egui_wants_pointer_input` renamed** — `wants_keyboard_input` and `wants_pointer_input` were renamed to `egui_wants_keyboard_input` and `egui_wants_pointer_input` in 0.34 to clarify they refer to egui's consumption (not the app's).

### 0.33 Changes
- **`egui::Plugin` trait** — replaces `Context::on_begin_pass` / `on_end_pass` callbacks with a structured trait-based plugin API. State lives on the plugin struct, persisting across frames.
- **`ImageButton` deprecated** — use `Button::new(image)` or `Button::new((image, text))` with atoms instead.
- **`screen_rect` deprecated** — `ctx.screen_rect()` is deprecated; prefer `ctx.content_rect()` for safe-area-aware placement (important on iOS). `ctx.viewport_rect()` gives the full native window rect.
- **Default text size increased** from 12.5 pt to 13.0 pt. Layouts that depend on exact text heights may need adjustment.
- **`deadlock_detection` feature removed** — deadlock detection is now always enabled in debug builds (panics after 30 s). `egui::Mutex` will now timeout after ~30 s in debug builds; switch to std/parking\_lot mutex for long-held locks.
- **`log` feature removed** — egui now always uses the `log` crate; the feature flag was unnecessary.
- **`SurrenderFocusOn` added** — new enum controlling when a focused widget releases keyboard focus.
- **`Memory::move_focus` added** — programmatically move keyboard focus in a direction.
- **`Plugin::on_widget_under_pointer` added** (0.33.2) — hook called with the widget currently under the pointer; useful for widget inspectors.
- **`Response::total_drag_delta` added** (0.33.2) — total accumulated drag delta over a drag gesture.
- **MSRV bumped** — 0.33 requires Rust 1.88; 0.34 requires Rust 1.92.
- **Improved kerning** — text may reflow slightly due to more accurate kerning calculations.

---

*Sources: [docs.rs/egui 0.34.2](https://docs.rs/egui/0.34.2/egui/) · [docs.rs/eframe 0.34.2](https://docs.rs/eframe/0.34.2/eframe/) · [CHANGELOG](https://github.com/emilk/egui/blob/main/CHANGELOG.md) · [egui_kittest](https://docs.rs/egui_kittest/latest/egui_kittest/)*