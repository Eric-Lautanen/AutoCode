# egui 0.34.3 API Reference (condensed)

One function per line: `Type::method(signature)` — short description.
Source: egui/epaint/emath crates, `main` branch matching release 0.34.3 (2026-05-27). Generated for token-efficient LLM/agent lookup, not exhaustive prose docs.

## Ui — core widget placement

**Ui**
- `.add(&mut self, widget: impl Widget) -> Response` — Add a Widget to this Ui at a location dependent on the current Layout.
- `.add_enabled(&mut self, enabled: bool, widget: impl Widget) -> Response` — Add a single Widget that is possibly disabled, i.e. greyed out and non-interactive.
- `.add_enabled_ui<R>(&mut self, enabled: bool, add_contents: impl FnOnce(&mut Ui) -> R,) -> InnerResponse<R>` — Add a section that is possibly disabled, i.e. greyed out and non-interactive.
- `.add_sized(&mut self, max_size: impl Into<Vec2>, widget: impl Widget) -> Response` — Add a Widget to this Ui with a given size.
- `.add_space(&mut self, amount: f32)` — Add extra space before the next widget.
- `.add_visible(&mut self, visible: bool, widget: impl Widget) -> Response` — Add a single Widget that is possibly invisible.
- `.advance_cursor_after_rect(&mut self, rect: Rect) -> Id` — Allocate a rect without interacting with it.
- `.allocate_at_least(&mut self, desired_size: Vec2, sense: Sense) -> (Rect, Response)` — Allocate at least as much space as needed, and interact with that rect.
- `.allocate_exact_size(&mut self, desired_size: Vec2, sense: Sense) -> (Rect, Response)` — Returns a Rect with exactly what you asked for.
- `.allocate_painter(&mut self, desired_size: Vec2, sense: Sense) -> (Response, Painter)` — Convenience function to get a region to paint on.
- `.allocate_rect(&mut self, rect: Rect, sense: Sense) -> Response` — Allocate a specific part of the Ui.
- `.allocate_response(&mut self, desired_size: Vec2, sense: Sense) -> Response` — Allocate space for a widget and check for interaction in the space.
- `.allocate_space(&mut self, desired_size: Vec2) -> (Id, Rect)` — Reserve this much space and move the cursor.
- `.allocate_ui<R>(&mut self, desired_size: Vec2, add_contents: impl FnOnce(&mut Self) -> R,) -> InnerResponse<R>` — Allocated the given space and then adds content to that space.
- `.allocate_ui_with_layout<R>(&mut self, desired_size: Vec2, layout: Layout, add_contents: impl FnOnce(&mut Self) -> R,) -> InnerResponse<R>` — Allocated the given space and then adds content to that space.
- `.auto_id_with(&self, id_salt: impl AsIdSalt) -> Id` — Same as ui.next_auto_id().with(id_salt)
- `.available_height(&self) -> f32` — The available height at the moment, given the current cursor.
- `.available_rect_before_wrap(&self) -> Rect` — In case of a wrapping layout, how much space is left on this row/column?
- `.available_size(&self) -> Vec2` — The available space at the moment, given the current cursor.
- `.available_size_before_wrap(&self) -> Vec2` — In case of a wrapping layout, how much space is left on this row/column?
- `.available_width(&self) -> f32` — The available width at the moment, given the current cursor.
- `.button<'a>(&mut self, atoms: impl IntoAtoms<'a>) -> Response` — Usage: if ui.button("Click me").clicked() { … }
- `.centered_and_justified<R>(&mut self, add_contents: impl FnOnce(&mut Self) -> R,) -> InnerResponse<R>` — This will make the next added widget centered and justified in the available space.
- `.checkbox<'a>(&mut self, checked: &'a mut bool, atoms: impl IntoAtoms<'a>) -> Response` — Show a checkbox.
- `.clip_rect(&self) -> Rect` — Screen-space rectangle for clipping what we paint in this ui.
- `.close(&self)` — Find and close the first closable parent.
- `.close_kind(&self, ui_kind: UiKind)` — Find and close the first closable parent of a specific UiKind.
- `.code(&mut self, text: impl Into<RichText>) -> Response` — Show text as monospace with a gray background.
- `.code_editor<S: widgets::text_edit::TextBuffer>(&mut self, text: &mut S) -> Response` — A TextEdit for code editing.
- `.collapsing<R>(&mut self, heading: impl Into<WidgetText>, add_contents: impl FnOnce(&mut Ui) -> R,) -> CollapsingResponse<R>` — A CollapsingHeader that starts out collapsed.
- `.color_edit_button_hsva(&mut self, hsva: &mut Hsva) -> Response` — Shows a button with the given color.
- `.color_edit_button_rgb(&mut self, rgb: &mut [f32; 3]) -> Response` — Shows a button with the given color.
- `.color_edit_button_rgba_premultiplied(&mut self, rgba_premul: &mut [f32; 4]) -> Response` — Shows a button with the given color.
- `.color_edit_button_rgba_unmultiplied(&mut self, rgba_unmul: &mut [f32; 4]) -> Response` — Shows a button with the given color.
- `.color_edit_button_srgb(&mut self, srgb: &mut [u8; 3]) -> Response` — Shows a button with the given color.
- `.color_edit_button_srgba(&mut self, srgba: &mut Color32) -> Response` — Shows a button with the given color.
- `.color_edit_button_srgba_premultiplied(&mut self, srgba: &mut [u8; 4]) -> Response` — Shows a button with the given color.
- `.color_edit_button_srgba_unmultiplied(&mut self, srgba: &mut [u8; 4]) -> Response` — Shows a button with the given color.
- `.colored_label(&mut self, color: impl Into<Color32>, text: impl Into<RichText>,) -> Response` — Show colored text.
- `.columns<R>(&mut self, num_columns: usize, add_contents: impl FnOnce(&mut [Self]) -> R,) -> R` — Temporarily split a Ui into several columns.
- `.columns_const<const NUM_COL: usize, R>(&mut self, add_contents: impl FnOnce(&mut [Self; NUM_COL]) -> R,) -> R` — Temporarily split a Ui into several columns.
- `.ctx(&self) -> &Context` — Get a reference to the parent Context.
- `.cursor(&self) -> Rect` — Where the next widget will be put.
- `.debug_paint_cursor(&self)` — Shows where the next widget is going to be placed
- `.disable(&mut self)` — Calling disable() will cause the Ui to deny all future interaction and all the widgets will draw with a gray look.
- `.dnd_drag_source<Payload, R>(&mut self, id: Id, payload: Payload, add_contents: impl FnOnce(&mut Self) -> R,) -> InnerResponse<R> where Payload: Any + Send + Sync,` — Create something that can be drag-and-dropped.
- `.dnd_drop_zone<Payload, R>(&mut self, frame: Frame, add_contents: impl FnOnce(&mut Ui) -> R,) -> (InnerResponse<R>, Option<Arc<Payload>>) where Payload: Any + Send + Sync,` — Surround the given ui with a frame which changes colors when you can drop something onto it.
- `.drag_angle(&mut self, radians: &mut f32) -> Response` — Modify an angle.
- `.drag_angle_tau(&mut self, radians: &mut f32) -> Response` — Modify an angle.
- `.end_row(&mut self)` — Move to the next row in a grid layout or wrapping layout.
- `.expand_to_include_rect(&mut self, rect: Rect)` — Expand the min_rect and max_rect of this ui to include a child at the given rect.
- `.expand_to_include_x(&mut self, x: f32)` — Ensure we are big enough to contain the given x-coordinate.
- `.expand_to_include_y(&mut self, y: f32)` — Ensure we are big enough to contain the given y-coordinate.
- `.group<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>` — Put into a Frame::group, visually grouping the contents together
- `.heading(&mut self, text: impl Into<RichText>) -> Response` — Show large text.
- `.horizontal<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>` — Start a ui with horizontal layout.
- `.horizontal_centered<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R,) -> InnerResponse<R>` — Like Self::horizontal, but allocates the full vertical height and then centers elements vertically.
- `.horizontal_top<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R,) -> InnerResponse<R>` — Like Self::horizontal, but aligns content with top.
- `.horizontal_wrapped<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R,) -> InnerResponse<R>` — Start a ui with horizontal layout that wraps to a new row when it reaches the right edge of the max_size.
- `.hyperlink(&mut self, url: impl ToString) -> Response` — Link to a web page.
- `.hyperlink_to(&mut self, label: impl Into<WidgetText>, url: impl ToString) -> Response` — Shortcut for add(Hyperlink::from_label_and_url(label, url)).
- `.id(&self) -> Id` — Generated based on id of parent ui together with an optional id salt.
- `.image<'a>(&mut self, source: impl Into<ImageSource<'a>>) -> Response` — Show an image available at the given uri.
- `.indent<R>(&mut self, id_salt: impl AsIdSalt, add_contents: impl FnOnce(&mut Ui) -> R,) -> InnerResponse<R>` — Create a child ui which is indented to the right.
- `.interact(&self, rect: Rect, id: Id, sense: Sense) -> Response` — Check for clicks, drags and/or hover on a specific region of this Ui.
- `.interact_opt(&self, rect: Rect, id: Id, sense: Sense, options: crate::InteractOptions,) -> Response` — Check for clicks, drags and/or hover on a specific region of this Ui.
- `.is_enabled(&self) -> bool` — If false, the Ui does not allow any interaction and the widgets in it will draw with a gray look.
- `.is_rect_visible(&self, rect: Rect) -> bool` — Can be used for culling: if false, then no part of rect will be visible on screen.
- `.is_sizing_pass(&self) -> bool` — Set to true in special cases where we do one frame where we size up the contents of the Ui, without actually showing it.
- `.is_tooltip(&self) -> bool` — Is this Ui in a tooltip?
- `.is_visible(&self) -> bool` — If false, any widgets added to the Ui will be invisible and non-interactive.
- `.label(&mut self, text: impl Into<WidgetText>) -> Response` — Show some text.
- `.layer_id(&self) -> LayerId` — Use this to paint stuff within this Ui.
- `.layout(&self) -> &Layout` — Read the Layout.
- `.link(&mut self, text: impl Into<WidgetText>) -> Response` — Looks like a hyperlink.
- `.make_persistent_id(&self, id_salt: impl AsIdSalt) -> Id` — Use this to generate widget ids for widgets that have persistent state in Memory.
- `.max_rect(&self) -> Rect` — New widgets will *try* to fit within this rectangle.
- `.menu_button<'a, R>(&mut self, atoms: impl IntoAtoms<'a>, add_contents: impl FnOnce(&mut Ui) -> R,) -> InnerResponse<Option<R>>` — Create a menu button that when clicked will show the given menu.
- `.menu_image_button<'a, R>(&mut self, image: impl Into<Image<'a>>, add_contents: impl FnOnce(&mut Ui) -> R,) -> InnerResponse<Option<R>>` — Create a menu button with an image that when clicked will show the given menu.
- `.menu_image_text_button<'a, R>(&mut self, image: impl Into<Image<'a>>, title: impl Into<WidgetText>, add_contents: impl FnOnce(&mut Ui) -> R,) -> InnerResponse<Option<R>>` — Create a menu button with an image and a text that when clicked will show the given menu.
- `.min_rect(&self) -> Rect` — Where and how large the Ui is already.
- `.min_size(&self) -> Vec2` — Size of content; same as min_rect().size()
- `.monospace(&mut self, text: impl Into<RichText>) -> Response` — Show monospace (fixed width) text.
- `.multiply_opacity(&mut self, opacity: f32)` — Like Self::set_opacity, but multiplies the given value with the current opacity.
- `.new(ctx: Context, id: Id, ui_builder: UiBuilder) -> Self` — Create a new top-level Ui.
- `.new_child(&mut self, ui_builder: UiBuilder) -> Self` — Create a child Ui with the properties of the given builder.
- `.next_auto_id(&self) -> Id` — This is the Id that will be assigned to the next widget added to this Ui.
- `.next_widget_position(&self) -> Pos2` — Where do we expect a zero-sized widget to be placed?
- `.opacity(&self) -> f32` — Read the current opacity of the underlying painter.
- `.painter(&self) -> &Painter` — Use this to paint stuff within this Ui.
- `.painter_at(&self, rect: Rect) -> Painter` — Create a painter for a sub-region of this Ui.
- `.pixels_per_point(&self) -> f32` — Number of physical pixels for each logical UI point.
- `.place(&mut self, max_rect: Rect, widget: impl Widget) -> Response` — Add a Widget to this Ui at a specific location (manual layout) without affecting this Uis cursor.
- `.push_id<R>(&mut self, id_salt: impl AsIdSalt, add_contents: impl FnOnce(&mut Ui) -> R,) -> InnerResponse<R>` — Create a child Ui with an explicit Id.
- `.put(&mut self, max_rect: Rect, widget: impl Widget) -> Response` — Add a Widget to this Ui at a specific location (manual layout) and advance the cursor after the widget.
- `.radio<'a>(&mut self, selected: bool, atoms: impl IntoAtoms<'a>) -> Response` — Show a RadioButton.
- `.radio_value<'a, Value: PartialEq>(&mut self, current_value: &mut Value, alternative: Value, atoms: impl IntoAtoms<'a>,) -> Response` — Show a RadioButton.
- `.rect_contains_pointer(&self, rect: Rect) -> bool` — Is the pointer (mouse/touch) above this rectangle in this Ui?
- `.reset_style(&mut self)` — Reset to the default style set in Context.
- `.response(&self) -> Response` — Read the Ui's background Response.
- `.scope<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>` — Create a scoped child ui.
- `.scope_builder<R>(&mut self, ui_builder: UiBuilder, add_contents: impl FnOnce(&mut Ui) -> R,) -> InnerResponse<R>` — Create a scoped child ui, inheriting properties from the parent as specified by the UiBuilder.
- `.scope_dyn<'c, R>(&mut self, ui_builder: UiBuilder, add_contents: Box<dyn FnOnce(&mut Ui) -> R + 'c>,) -> InnerResponse<R>` — Self::scope_builder but with dynamic dispatch.
- `.scroll_to_cursor(&self, align: Option<Align>)` — Adjust the scroll position of any parent crate::ScrollArea so that the cursor (where the next widget goes) becomes visible.
- `.scroll_to_cursor_animation(&self, align: Option<Align>, animation: style::ScrollAnimation,)` — Same as Self::scroll_to_cursor, but allows you to specify the style::ScrollAnimation.
- `.scroll_to_rect(&self, rect: Rect, align: Option<Align>)` — Adjust the scroll position of any parent crate::ScrollArea so that the given Rect becomes visible.
- `.scroll_to_rect_animation(&self, rect: Rect, align: Option<Align>, animation: style::ScrollAnimation,)` — Same as Self::scroll_to_rect, but allows you to specify the style::ScrollAnimation.
- `.scroll_with_delta(&self, delta: Vec2)` — Scroll this many points in the given direction, in the parent crate::ScrollArea.
- `.scroll_with_delta_animation(&self, delta: Vec2, animation: style::ScrollAnimation)` — Same as Self::scroll_with_delta, but allows you to specify the style::ScrollAnimation.
- `.selectable_label<'a>(&mut self, checked: bool, text: impl IntoAtoms<'a>) -> Response` — Show a label which can be selected or not.
- `.selectable_value<'a, Value: PartialEq>(&mut self, current_value: &mut Value, selected_value: Value, text: impl IntoAtoms<'a>,) -> Response` — Show selectable text.
- `.separator(&mut self) -> Response` — Shortcut for add(Separator::default())
- `.set_clip_rect(&mut self, clip_rect: Rect)` — Screen-space rectangle for clipping what we paint in this ui.
- `.set_height(&mut self, height: f32)` — Set both the minimum and maximum height.
- `.set_height_range(&mut self, height: impl Into<Rangef>)` — ui.set_height_range(min..=max); is equivalent to ui.set_min_height(min); ui.set_max_height(max);.
- `.set_invisible(&mut self)` — Calling set_invisible() will cause all further widgets to be invisible, yet still allocate space.
- `.set_max_height(&mut self, height: f32)` — Set the maximum height of the ui.
- `.set_max_size(&mut self, size: Vec2)` — Set the maximum size of the ui.
- `.set_max_width(&mut self, width: f32)` — Set the maximum width of the ui.
- `.set_min_height(&mut self, height: f32)` — Set the minimum height of the ui.
- `.set_min_size(&mut self, size: Vec2)` — Set the minimum size of the ui.
- `.set_min_width(&mut self, width: f32)` — Set the minimum width of the ui.
- `.set_opacity(&mut self, opacity: f32)` — Make the widget in this Ui semi-transparent.
- `.set_row_height(&mut self, height: f32)` — Set row height in horizontal wrapping layout.
- `.set_style(&mut self, style: impl Into<Arc<Style>>)` — Changes apply to this Ui and its subsequent children.
- `.set_width(&mut self, width: f32)` — Set both the minimum and maximum width.
- `.set_width_range(&mut self, width: impl Into<Rangef>)` — ui.set_width_range(min..=max); is equivalent to ui.set_min_width(min); ui.set_max_width(max);.
- `.should_close(&self) -> bool` — Was Ui::close called on this Ui or any of its children? Only works if the Ui was created with UiBuilder::closable.
- `.shrink_clip_rect(&mut self, new_clip_rect: Rect)` — Constrain the rectangle in which we can paint.
- `.shrink_height_to_current(&mut self)` — Helper: shrinks the max height to the current height, so further widgets will try not to be taller than previous widgets.
- `.shrink_width_to_current(&mut self)` — Helper: shrinks the max width to the current width, so further widgets will try not to be wider than previous widgets.
- `.skip_ahead_auto_ids(&mut self, count: usize)` — Pretend like count widgets have been allocated.
- `.small(&mut self, text: impl Into<RichText>) -> Response` — Show small text.
- `.small_button<'a>(&mut self, atoms: impl IntoAtoms<'a>) -> Response` — A button as small as normal body text.
- `.spacing(&self) -> &crate::style::Spacing` — The current spacing options for this Ui.
- `.spacing_mut(&mut self) -> &mut crate::style::Spacing` — Mutably borrow internal Spacing.
- `.spinner(&mut self) -> Response` — Shortcut for add(Spinner::new())
- `.stack(&self) -> &Arc<UiStack>` — Get a reference to this Ui's UiStack.
- `.strong(&mut self, text: impl Into<RichText>) -> Response` — Show text that stand out a bit (e.g. slightly brighter).
- `.style(&self) -> &Arc<Style>` — Style options for this Ui and its children.
- `.style_mut(&mut self) -> &mut Style` — Mutably borrow internal Style.
- `.take_available_height(&mut self)` — Makes the ui always fill up the available space in the y axis.
- `.take_available_space(&mut self)` — Makes the ui always fill up the available space.
- `.take_available_width(&mut self)` — Makes the ui always fill up the available space in the x axis.
- `.text_edit_multiline<S: widgets::text_edit::TextBuffer>(&mut self, text: &mut S,) -> Response` — A TextEdit for multiple lines.
- `.text_edit_singleline<S: widgets::text_edit::TextBuffer>(&mut self, text: &mut S,) -> Response` — No newlines (\n) allowed.
- `.text_style_height(&self, style: &TextStyle) -> f32` — The height of text of this text style.
- `.text_valign(&self) -> Align` — How to vertically align text
- `.toggle_value<'a>(&mut self, selected: &mut bool, atoms: impl IntoAtoms<'a>) -> Response` — Acts like a checkbox, but looks like a Button::selectable.
- `.ui_contains_pointer(&self) -> bool` — Is the pointer (mouse/touch) above the current Ui?
- `.unique_id(&self) -> Id` — This is a globally unique ID of this Ui, based on where in the hierarchy of widgets this Ui is in.
- `.vertical<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>` — Start a ui with vertical layout.
- `.vertical_centered<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R,) -> InnerResponse<R>` — Start a ui with vertical layout.
- `.vertical_centered_justified<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R,) -> InnerResponse<R>` — Start a ui with vertical layout.
- `.visuals(&self) -> &crate::Visuals` — The current visuals settings of this Ui.
- `.visuals_mut(&mut self) -> &mut crate::Visuals` — Mutably borrow internal visuals.
- `.weak(&mut self, text: impl Into<RichText>) -> Response` — Show text that is weaker (fainter color).
- `.will_parent_close(&self) -> bool` — Will this Ui or any of its parents close this frame?
- `.with_layout<R>(&mut self, layout: Layout, add_contents: impl FnOnce(&mut Self) -> R,) -> InnerResponse<R>` — The new layout will take up all available space.
- `.with_visual_transform<R>(&mut self, transform: emath::TSTransform, add_contents: impl FnOnce(&mut Self) -> R,) -> InnerResponse<R>` — Create a new Scope and transform its contents via a emath::TSTransform.
- `.wrap_mode(&self) -> TextWrapMode` — Which wrap mode should the text use in this Ui?

## Context — app/frame-level handle

**Context**
- `.accesskit_node_builder<R>(&self, id: Id, writer: impl FnOnce(&mut accesskit::Node) -> R,) -> Option<R>` — If AccessKit support is active for the current frame, get or create a node builder with the specified ID and return a mutable reference to it.
- `.add_bytes_loader(&self, loader: Arc<dyn load::BytesLoader + Send + Sync + 'static>)` — Add a new bytes loader.
- `.add_font(&self, new_font: FontInsert)` — Tell egui which fonts to use.
- `.add_image_loader(&self, loader: Arc<dyn load::ImageLoader + Send + Sync + 'static>)` — Add a new image loader.
- `.add_plugin(&self, plugin: impl plugin::Plugin + 'static)` — Register a Plugin
- `.add_texture_loader(&self, loader: Arc<dyn load::TextureLoader + Send + Sync + 'static>)` — Add a new texture loader.
- `.all_styles_mut(&self, mut mutate_style: impl FnMut(&mut Style))` — Mutate the Styles used by all subsequent popups, menus, etc. in both dark and light mode.
- `.animate_bool(&self, id: Id, value: bool) -> f32` — Returns a value in the range [0, 1], to indicate "how on" this thing is.
- `.animate_bool_responsive(&self, id: Id, value: bool) -> f32` — Like Self::animate_bool, but uses an easing function that makes the value move quickly in the beginning and slow down towards the end.
- `.animate_bool_with_easing(&self, id: Id, value: bool, easing: fn(f32) -> f32) -> f32` — Like Self::animate_bool but allows you to control the easing function.
- `.animate_bool_with_time(&self, id: Id, target_value: bool, animation_time: f32) -> f32` — Like Self::animate_bool but allows you to control the animation time.
- `.animate_bool_with_time_and_easing(&self, id: Id, target_value: bool, animation_time: f32, easing: fn(f32) -> f32,) -> f32` — Like Self::animate_bool but allows you to control the animation time and easing function.
- `.animate_value_with_time(&self, id: Id, target_value: f32, animation_time: f32) -> f32` — Smoothly animate an f32 value.
- `.any_popup_open(&self) -> bool` — Is a popup or (context) menu open?
- `.begin_pass(&self, mut new_input: RawInput)` — An alternative to calling Self::run_ui.
- `.check_for_id_clash(&self, id: Id, new_rect: Rect, what: &str)` — If the given Id has been used previously the same pass at different position, then an error will be printed on screen.
- `.clear_animations(&self)` — Clear memory of any animations.
- `.content_rect(&self) -> Rect` — Returns the position and size of the egui area that is safe for content rendering.
- `.copy_image(&self, image: crate::ColorImage)` — Copy the given image to the system clipboard.
- `.copy_text(&self, text: String)` — Copy the given text to the system clipboard.
- `.cumulative_frame_nr(&self) -> u64` — The total number of completed frames.
- `.cumulative_frame_nr_for(&self, id: ViewportId) -> u64` — The total number of completed frames.
- `.cumulative_pass_nr(&self) -> u64` — The total number of completed passes (usually there is one pass per rendered frame).
- `.cumulative_pass_nr_for(&self, id: ViewportId) -> u64` — The total number of completed passes (usually there is one pass per rendered frame).
- `.current_pass_index(&self) -> usize` — The index of the current pass in the current frame, starting at zero.
- `.data<R>(&self, reader: impl FnOnce(&IdTypeMap) -> R) -> R` — Read-only access to IdTypeMap, which stores superficial widget state.
- `.data_mut<R>(&self, writer: impl FnOnce(&mut IdTypeMap) -> R) -> R` — Read-write access to IdTypeMap, which stores superficial widget state.
- `.debug_on_hover(&self) -> bool` — Whether or not to debug widget layout on hover.
- `.debug_painter(&self) -> Painter` — Paint on top of _everything_ else (even on top of tooltips and popups).
- `.debug_text(&self, text: impl Into<WidgetText>)` — Print this text next to the cursor at the end of the pass.
- `.disable_accesskit(&self)` — Disable generation of AccessKit tree updates in all future frames.
- `.drag_started_id(&self) -> Option<Id>` — This widget just started being dragged this pass.
- `.drag_stopped_id(&self) -> Option<Id>` — This widget was being dragged, but was released this pass.
- `.dragged_id(&self) -> Option<Id>` — The widget currently being dragged, if any.
- `.dragging_something_else(&self, not_this: Id) -> bool` — Is something else being dragged?
- `.egui_is_using_pointer(&self) -> bool` — Is egui currently using the pointer position (e.g. dragging a slider)?
- `.egui_wants_keyboard_input(&self) -> bool` — If true, egui is currently listening on text input (e.g. typing text in a crate::TextEdit).
- `.egui_wants_pointer_input(&self) -> bool` — True if egui is currently interested in the pointer (mouse or touch).
- `.embed_viewports(&self) -> bool` — If true, Self::show_viewport_deferred and Self::show_viewport_immediate will embed the new viewports inside the existing one, instead of spawning a new nativ...
- `.enable_accesskit(&self)` — Enable generation of AccessKit tree updates in all future frames.
- `.end_pass(&self) -> FullOutput` — Call at the end of each frame if you called Context::begin_pass.
- `.fonts<R>(&self, reader: impl FnOnce(&FontsView<'_>) -> R) -> R` — Read-only access to Fonts.
- `.fonts_mut<R>(&self, reader: impl FnOnce(&mut FontsView<'_>) -> R) -> R` — Read-write access to Fonts.
- `.forget_all_images(&self)` — Release all memory and textures related to images used in Ui::image or crate::Image.
- `.forget_image(&self, uri: &str)` — Release all memory and textures related to the given image URI.
- `.format_modifiers(&self, modifiers: Modifiers) -> String` — Format the given modifiers in a human-readable way (e.g. Ctrl+Shift+X).
- `.format_shortcut(&self, shortcut: &KeyboardShortcut) -> String` — Format the given shortcut in a human-readable way (e.g. Ctrl+Shift+X).
- `.global_style(&self) -> Arc<Style>` — The currently active Style used by all subsequent popups, menus, etc.
- `.global_style_mut(&self, mutate_style: impl FnOnce(&mut Style))` — Mutate the currently active Style used by all subsequent popups, menus, etc. Use Self::all_styles_mut to mutate both dark and light mode styles.
- `.globally_used_rect(&self) -> Rect` — How much space is used by windows and the top-level Ui.
- `.graphics<R>(&self, reader: impl FnOnce(&GraphicLayers) -> R) -> R` — Read-only access to GraphicLayers, where painted crate::Shapes are written to.
- `.graphics_mut<R>(&self, writer: impl FnOnce(&mut GraphicLayers) -> R) -> R` — Read-write access to GraphicLayers, where painted crate::Shapes are written to.
- `.has_pending_images(&self) -> bool` — Returns true if any image is currently being loaded.
- `.has_requested_repaint(&self) -> bool` — Has a repaint been requested for the current viewport?
- `.has_requested_repaint_for(&self, viewport_id: &ViewportId) -> bool` — Has a repaint been requested for the given viewport?
- `.highlight_widget(&self, id: Id)` — Highlight this widget, to make it look like it is hovered, even if it isn't.
- `.include_bytes(&self, uri: impl Into<Cow<'static, str>>, bytes: impl Into<Bytes>)` — Associate some static bytes with a uri.
- `.input<R>(&self, reader: impl FnOnce(&InputState) -> R) -> R` — Read-only access to InputState.
- `.input_for<R>(&self, id: ViewportId, reader: impl FnOnce(&InputState) -> R) -> R` — This will create a InputState::default() if there is no input state for that viewport
- `.input_mut<R>(&self, writer: impl FnOnce(&mut InputState) -> R) -> R` — Read-write access to InputState.
- `.input_mut_for<R>(&self, id: ViewportId, writer: impl FnOnce(&mut InputState) -> R) -> R` — This will create a InputState::default() if there is no input state for that viewport
- `.inspection_ui(&self, ui: &mut Ui)` — Show the state of egui, including its input and output.
- `.interaction_snapshot<R>(&self, reader: impl FnOnce(&InteractionSnapshot) -> R) -> R` — Read which widgets are currently being interacted with.
- `.interactive_rects_last_pass(&self) -> Vec<Rect>` — Rectangles that could receive pointer input in the last completed pass.
- `.is_being_dragged(&self, id: Id) -> bool` — Is this specific widget being dragged?
- `.is_loader_installed(&self, id: &str) -> bool` — Returns true if the chain of bytes, image, or texture loaders contains a loader with the given id.
- `.is_pointer_over_egui(&self) -> bool` — Is the pointer (mouse/touch) over any egui area?
- `.layer_id_at(&self, pos: Pos2) -> Option<LayerId>` — Top-most layer at the given position.
- `.layer_painter(&self, layer_id: LayerId) -> Painter` — Get a full-screen painter for a new or existing layer
- `.layer_transform_from_global(&self, layer_id: LayerId) -> Option<TSTransform>` — Return how to transform the graphics of the global coordinate system into the local coordinate system of the given layer.
- `.layer_transform_to_global(&self, layer_id: LayerId) -> Option<TSTransform>` — Return how to transform the graphics of the given layer into the global coordinate system.
- `.load_texture(&self, name: impl Into<String>, image: impl Into<ImageData>, options: TextureOptions,) -> TextureHandle` — Allocate a texture.
- `.loaders(&self) -> Arc<Loaders>` — The loaders of bytes, images, and textures.
- `.loaders_ui(&self, ui: &mut crate::Ui)` — Show stats about different image loaders.
- `.memory<R>(&self, reader: impl FnOnce(&Memory) -> R) -> R` — Read-only access to Memory.
- `.memory_mut<R>(&self, writer: impl FnOnce(&mut Memory) -> R) -> R` — Read-write access to Memory.
- `.memory_ui(&self, ui: &mut crate::Ui)` — Shows the contents of Self::memory.
- `.move_to_top(&self, layer_id: LayerId)` — Moves the given area to the top in its Order.
- `.multi_touch(&self) -> Option<MultiTouchInfo>` — Calls InputState::multi_touch.
- `.native_pixels_per_point(&self) -> Option<f32>` — The number of physical pixels for each logical point on this monitor.
- `.on_begin_pass(&self, debug_name: &'static str, cb: plugin::ContextCallback)` — Call the given callback at the start of each pass of each viewport.
- `.on_end_pass(&self, debug_name: &'static str, cb: plugin::ContextCallback)` — Call the given callback at the end of each pass of each viewport.
- `.open_url(&self, open_url: crate::OpenUrl)` — Open an URL in a browser.
- `.options<R>(&self, reader: impl FnOnce(&Options) -> R) -> R` — Read-only access to Options.
- `.options_mut<R>(&self, writer: impl FnOnce(&mut Options) -> R) -> R` — Read-write access to Options.
- `.os(&self) -> OperatingSystem` — What operating system are we running on?
- `.output<R>(&self, reader: impl FnOnce(&PlatformOutput) -> R) -> R` — Read-only access to PlatformOutput.
- `.output_mut<R>(&self, writer: impl FnOnce(&mut PlatformOutput) -> R) -> R` — Read-write access to PlatformOutput.
- `.parent_viewport_id(&self) -> ViewportId` — Return the ViewportId of his parent.
- `.pixels_per_point(&self) -> f32` — The number of physical pixels for each logical point.
- `.plugin<T: plugin::Plugin>(&self) -> TypedPluginHandle<T>` — Get a handle to the plugin of type T.
- `.plugin_opt<T: plugin::Plugin>(&self) -> Option<TypedPluginHandle<T>>` — Get a handle to the plugin of type T, if it was registered.
- `.plugin_or_default<T: plugin::Plugin + Default>(&self) -> TypedPluginHandle<T>` — Get a handle to the plugin of type T, or insert its default.
- `.pointer_hover_pos(&self) -> Option<Pos2>` — If it is a good idea to show a tooltip, where is pointer?
- `.pointer_interact_pos(&self) -> Option<Pos2>` — If you detect a click or drag and want to know where it happened, use this.
- `.pointer_latest_pos(&self) -> Option<Pos2>` — Latest reported pointer position.
- `.read_response(&self, id: Id) -> Option<Response>` — Read the response of some widget, which may be called _before_ creating the widget (!).
- `.rect_contains_pointer(&self, layer_id: LayerId, rect: Rect) -> bool` — Does the given rectangle contain the mouse pointer?
- `.register_widget_info(&self, id: Id, make_info: impl Fn() -> crate::WidgetInfo)` — This is called by Response::widget_info, but can also be called directly.
- `.repaint_causes(&self) -> Vec<RepaintCause>` — Why are we repainting?
- `.request_discard(&self, reason: impl Into<Cow<'static, str>>)` — Request to discard the visual output of this pass, and to immediately do another one.
- `.request_repaint(&self)` — Call this if there is need to repaint the UI, i.e. if you are showing an animation.
- `.request_repaint_after(&self, duration: Duration)` — Request repaint after at most the specified duration elapses.
- `.request_repaint_after_for(&self, duration: Duration, id: ViewportId)` — Request repaint after at most the specified duration elapses.
- `.request_repaint_after_secs(&self, seconds: f32)` — Repaint after this many seconds.
- `.request_repaint_of(&self, id: ViewportId)` — Call this if there is need to repaint the UI, i.e. if you are showing an animation.
- `.requested_repaint_last_pass(&self) -> bool` — Was a repaint requested last pass for the current viewport?
- `.requested_repaint_last_pass_for(&self, viewport_id: &ViewportId) -> bool` — Was a repaint requested last pass for the given viewport?
- `.run_ui(&self, new_input: RawInput, mut run_ui: impl FnMut(&mut Ui)) -> FullOutput` — Run the ui code for one frame.
- `.send_cmd(&self, cmd: crate::OutputCommand)` — Add a command to PlatformOutput::commands, for the integration to execute at the end of the frame.
- `.send_viewport_cmd(&self, command: ViewportCommand)` — Send a command to the current viewport.
- `.send_viewport_cmd_to(&self, id: ViewportId, command: ViewportCommand)` — Send a command to a specific viewport.
- `.set_cursor_icon(&self, cursor_icon: CursorIcon)` — Set the cursor icon.
- `.set_cursor_image(&self, image: Option<crate::CustomCursorImage>)` — Request that the integration display this RGBA bitmap as the OS cursor for the next frame, instead of the standard cursor_icon.
- `.set_debug_on_hover(&self, debug_on_hover: bool)` — Turn on/off whether or not to debug widget layout on hover.
- `.set_dragged_id(&self, id: Id)` — Set which widget is being dragged.
- `.set_embed_viewports(&self, value: bool)` — If true, Self::show_viewport_deferred and Self::show_viewport_immediate will embed the new viewports inside the existing one, instead of spawning a new nativ...
- `.set_fonts(&self, font_definitions: FontDefinitions)` — Tell egui which fonts to use.
- `.set_global_style(&self, style: impl Into<Arc<Style>>)` — The currently active Style used by all new popups, menus, etc.
- `.set_immediate_viewport_renderer(callback: impl for<'a> Fn(&Self, ImmediateViewport<'a>) + 'static,)` — For integrations: Set this to render a sync viewport.
- `.set_os(&self, os: OperatingSystem)` — Set the operating system we are running on.
- `.set_pixels_per_point(&self, pixels_per_point: f32)` — Set the number of physical pixels for each logical point.
- `.set_request_repaint_callback(&self, callback: impl Fn(RequestRepaintInfo) + Send + Sync + 'static,)` — For integrations: this callback will be called when an egui user calls Self::request_repaint or Self::request_repaint_after.
- `.set_style_of(&self, theme: Theme, style: impl Into<Arc<Style>>)` — The Style used by all new popups, menus, etc. Use Self::set_theme to choose between dark and light mode.
- `.set_sublayer(&self, parent: LayerId, child: LayerId)` — Mark the child layer as a sublayer of parent.
- `.set_theme(&self, theme_preference: impl Into<crate::ThemePreference>)` — The Theme used to select between dark and light Self::global_style as the active style used by all subsequent popups, menus, etc.
- `.set_transform_layer(&self, layer_id: LayerId, transform: TSTransform)` — Transform the graphics of the given layer.
- `.set_visuals(&self, visuals: crate::Visuals)` — The crate::Visuals used by all subsequent popups, menus, etc.
- `.set_visuals_of(&self, theme: Theme, visuals: crate::Visuals)` — The crate::Visuals used by all subsequent popups, menus, etc.
- `.set_zoom_factor(&self, zoom_factor: f32)` — Sets zoom factor of the UI.
- `.settings_ui(&self, ui: &mut Ui)` — Show a ui for settings (style and tessellation options).
- `.show_viewport_deferred(&self, new_viewport_id: ViewportId, viewport_builder: ViewportBuilder, viewport_ui_cb: impl Fn(&mut Ui, ViewportClass) + Send + Sync + 'static,)` — Show a deferred viewport, creating a new native window, if possible.
- `.show_viewport_immediate<T>(&self, new_viewport_id: ViewportId, builder: ViewportBuilder, mut viewport_ui_cb: impl FnMut(&mut Ui, ViewportClass) -> T,) -> T` — Show an immediate viewport, creating a new native window, if possible.
- `.stop_dragging(&self)` — Stop dragging any widget.
- `.style_mut_of(&self, theme: Theme, mutate_style: impl FnOnce(&mut Style))` — Mutate the Style used by all subsequent popups, menus, etc.
- `.style_of(&self, theme: Theme) -> Arc<Style>` — The Style used by all subsequent popups, menus, etc.
- `.style_ui(&self, ui: &mut Ui, theme: Theme)` — Edit the Style.
- `.system_theme(&self) -> Option<Theme>` — Does the OS use dark or light mode? This is used when the theme preference is set to crate::ThemePreference::System.
- `.tessellate(&self, shapes: Vec<ClippedShape>, pixels_per_point: f32,) -> Vec<ClippedPrimitive>` — Tessellate the given shapes into triangle meshes.
- `.tessellation_options<R>(&self, reader: impl FnOnce(&TessellationOptions) -> R) -> R` — Read-only access to TessellationOptions.
- `.tessellation_options_mut<R>(&self, writer: impl FnOnce(&mut TessellationOptions) -> R,) -> R` — Read-write access to TessellationOptions.
- `.tex_manager(&self) -> Arc<RwLock<epaint::textures::TextureManager>>` — Low-level texture manager.
- `.text_edit_focused(&self) -> bool` — Is the currently focused widget a text edit?
- `.texture_ui(&self, ui: &mut crate::Ui)` — Show stats about the allocated textures.
- `.theme(&self) -> Theme` — The Theme used to select the appropriate Style (dark or light) used by all subsequent popups, menus, etc.
- `.time(&self) -> f64` — Current time in seconds, relative to some unknown epoch.
- `.top_layer_id(&self) -> Option<LayerId>` — Retrieve the LayerId of the top level windows.
- `.transform_layer_shapes(&self, layer_id: LayerId, transform: TSTransform)` — Transform all the graphics at the given layer.
- `.try_load_bytes(&self, uri: &str) -> load::BytesLoadResult` — Try loading the bytes from the given uri using any available bytes loaders.
- `.try_load_image(&self, uri: &str, size_hint: load::SizeHint) -> load::ImageLoadResult` — Try loading the image from the given uri using any available image loaders.
- `.try_load_texture(&self, uri: &str, texture_options: TextureOptions, size_hint: load::SizeHint,) -> load::TextureLoadResult` — Try loading the texture from the given uri using any available texture loaders.
- `.viewport<R>(&self, reader: impl FnOnce(&ViewportState) -> R) -> R` — Read the state of the current viewport.
- `.viewport_for<R>(&self, viewport_id: ViewportId, reader: impl FnOnce(&ViewportState) -> R,) -> R` — Read the state of a specific current viewport.
- `.viewport_id(&self) -> ViewportId` — Return the ViewportId of the current viewport.
- `.viewport_rect(&self) -> Rect` — Returns the position and size of the full area available to egui
- `.will_discard(&self) -> bool` — Will the visual output of this pass be discarded?
- `.with_plugin<T: plugin::Plugin + 'static, R>(&self, f: impl FnOnce(&mut T) -> R,) -> Option<R>` — Call the provided closure with the plugin of type T, if it was registered.
- `.zoom_factor(&self) -> f32` — Global zoom factor of the UI.

## Response & InnerResponse — interaction results

**Response**
- `.changed(&self) -> bool` — Was the underlying data changed?
- `.clicked(&self) -> bool` — Returns true if this widget was clicked this frame by the primary button.
- `.clicked_by(&self, button: PointerButton) -> bool` — Returns true if this widget was clicked this frame by the given mouse button.
- `.clicked_elsewhere(&self) -> bool` — true if there was a click *outside* the rect of this widget.
- `.clicked_with_open_in_background(&self) -> bool` — Was this widget middle-clicked or clicked while holding down a modifier key?
- `.contains_pointer(&self) -> bool` — Returns true if the pointer is contained by the response rect, and no other widget is covering it.
- `.context_menu(&self, add_contents: impl FnOnce(&mut Ui)) -> Option<InnerResponse<()>>` — Response to secondary clicks (right-clicks) by showing the given menu.
- `.context_menu_opened(&self) -> bool` — Returns whether a context menu is currently open for this widget.
- `.dnd_hover_payload<Payload: Any + Send + Sync>(&self) -> Option<Arc<Payload>>` — Drag-and-Drop: Return what is being held over this widget, if any.
- `.dnd_release_payload<Payload: Any + Send + Sync>(&self) -> Option<Arc<Payload>>` — Drag-and-Drop: Return what is being dropped onto this widget, if any.
- `.dnd_set_drag_payload<Payload: Any + Send + Sync>(&self, payload: Payload)` — If the user started dragging this widget this frame, store the payload for drag-and-drop.
- `.double_clicked(&self) -> bool` — Returns true if this widget was double-clicked this frame by the primary button.
- `.double_clicked_by(&self, button: PointerButton) -> bool` — Returns true if this widget was double-clicked this frame by the given button.
- `.drag_delta(&self) -> Vec2` — If dragged, how many points were we dragged in since last frame?
- `.drag_motion(&self) -> Vec2` — If dragged, how far did the mouse move since last frame?
- `.drag_started(&self) -> bool` — Did a drag on this widget begin this frame?
- `.drag_started_by(&self, button: PointerButton) -> bool` — Did a drag on this widget by the button begin this frame?
- `.drag_stopped(&self) -> bool` — The widget was being dragged, but now it has been released.
- `.drag_stopped_by(&self, button: PointerButton) -> bool` — The widget was being dragged by the button, but now it has been released.
- `.dragged(&self) -> bool` — The widget is being dragged.
- `.dragged_by(&self, button: PointerButton) -> bool` — See Self::dragged.
- `.enabled(&self) -> bool` — Was the widget enabled? If false, there was no interaction attempted and the widget should be drawn in a gray disabled look.
- `.gained_focus(&self) -> bool` — True if this widget has keyboard focus this frame, but didn't last frame.
- `.has_focus(&self) -> bool` — This widget has the keyboard focus (i.e. is receiving key presses).
- `.highlight(mut self) -> Self` — Highlight this widget, to make it look like it is hovered, even if it isn't.
- `.highlighted(&self) -> bool` — The widget is highlighted via a call to Self::highlight or Context::highlight_widget.
- `.hover_pos(&self) -> Option<Pos2>` — If it is a good idea to show a tooltip, where is pointer?
- `.hovered(&self) -> bool` — The pointer is hovering above this widget or the widget was clicked/tapped this frame.
- `.interact(&self, sense: Sense) -> Self` — Sense more interactions (e.g. sense clicks on a Response returned from a label).
- `.interact_pointer_pos(&self) -> Option<Pos2>` — Where the pointer (mouse/touch) were when this widget was clicked or dragged.
- `.intrinsic_size(&self) -> Option<Vec2>` — The intrinsic / desired size of the widget.
- `.is_pointer_button_down_on(&self) -> bool` — Is the pointer button currently down on this widget?
- `.is_tooltip_open(&self) -> bool` — Was the tooltip open last frame?
- `.labelled_by(self, id: Id) -> Self` — Associate a label with a control for accessibility.
- `.long_touched(&self) -> bool` — Was this long-pressed on a touch screen?
- `.lost_focus(&self) -> bool` — The widget had keyboard focus and lost it, either because the user pressed tab or clicked somewhere else, or (in case of a crate::TextEdit) because the user...
- `.mark_changed(&mut self)` — Report the data shown by this widget changed.
- `.middle_clicked(&self) -> bool` — Returns true if this widget was clicked this frame by the middle mouse button.
- `.on_disabled_hover_text(self, text: impl Into<WidgetText>) -> Self` — Show this text when hovering if the widget is disabled.
- `.on_disabled_hover_ui(self, add_contents: impl FnOnce(&mut Ui)) -> Self` — Show this UI when hovering if the widget is disabled.
- `.on_hover_and_drag_cursor(self, cursor: CursorIcon) -> Self` — When hovered or dragged, use this icon for the mouse cursor.
- `.on_hover_cursor(self, cursor: CursorIcon) -> Self` — When hovered, use this icon for the mouse cursor.
- `.on_hover_text(self, text: impl Into<WidgetText>) -> Self` — Show this text if the widget was hovered (i.e. a tooltip).
- `.on_hover_text_at_pointer(self, text: impl Into<WidgetText>) -> Self` — Like on_hover_text, but show the text next to cursor.
- `.on_hover_ui(self, add_contents: impl FnOnce(&mut Ui)) -> Self` — Show this UI if the widget was hovered (i.e. a tooltip).
- `.on_hover_ui_at_pointer(self, add_contents: impl FnOnce(&mut Ui)) -> Self` — Like on_hover_ui, but show the ui next to cursor.
- `.output_event(&self, event: crate::output::OutputEvent)`
- `.paint_debug_info(&self)` — Draw a debug rectangle over the response displaying the response's id and whether it is enabled and/or hovered.
- `.parent_id(&self) -> Id` — The Id of the parent crate::Ui that hosts this widget.
- `.request_focus(&self)` — Request that this widget get keyboard focus.
- `.scroll_to_me(&self, align: Option<Align>)` — Adjust the scroll position until this UI becomes visible.
- `.scroll_to_me_animation(&self, align: Option<Align>, animation: crate::style::ScrollAnimation,)` — Like Self::scroll_to_me, but allows you to specify the crate::style::ScrollAnimation.
- `.secondary_clicked(&self) -> bool` — Returns true if this widget was clicked this frame by the secondary mouse button (e.g. the right mouse button).
- `.set_close(&mut self)` — Set the Flags::CLOSE flag.
- `.set_intrinsic_size(&mut self, size: Vec2)` — Set the intrinsic / desired size of the widget.
- `.should_close(&self) -> bool` — Should the container be closed?
- `.show_tooltip_text(&self, text: impl Into<WidgetText>)` — Always show this tooltip, even if disabled and the user isn't hovering it.
- `.show_tooltip_ui(&self, add_contents: impl FnOnce(&mut Ui))` — Always show this tooltip, even if disabled and the user isn't hovering it.
- `.surrender_focus(&self)` — Surrender keyboard focus for this widget.
- `.total_drag_delta(&self) -> Option<Vec2>` — If dragged, how many points have we been dragged since the start of the drag?
- `.triple_clicked(&self) -> bool` — Returns true if this widget was triple-clicked this frame by the primary button.
- `.triple_clicked_by(&self, button: PointerButton) -> bool` — Returns true if this widget was triple-clicked this frame by the given button.
- `.union(&self, other: Self) -> Self` — A logical "or" operation.
- `.widget_info(&self, make_info: impl Fn() -> crate::WidgetInfo)` — For accessibility.
- `.widget_state(&self) -> WidgetState`
- `.with_new_rect(self, rect: Rect) -> Self` — Returns a response with a modified Self::rect.

**InnerResponse**
- `.new(inner: R, response: Response) -> Self`

**CollapsingResponse**
- `.fully_closed(&self) -> bool` — Was the CollapsingHeader fully closed (and not being animated)?
- `.fully_open(&self) -> bool` — Was the CollapsingHeader fully open (and not being animated)?

**ModalResponse**
- `.should_close(&self) -> bool` — Should the modal be closed? Returns true if: - the backdrop was clicked - this is the topmost modal, no popup is open and the escape key was pressed

**HeaderResponse**
- `.body<BodyRet>(mut self, add_body: impl FnOnce(&mut Ui) -> BodyRet,) -> (Response, InnerResponse<HeaderRet>, Option<InnerResponse<BodyRet>>,)` — Returns the response of the collapsing button, the custom header, and the custom body.
- `.body_unindented<BodyRet>(mut self, add_body: impl FnOnce(&mut Ui) -> BodyRet,) -> (Response, InnerResponse<HeaderRet>, Option<InnerResponse<BodyRet>>,)` — Returns the response of the collapsing button, the custom header, and the custom body, without indentation.
- `.is_open(&self) -> bool`
- `.set_open(&mut self, open: bool)`
- `.toggle(&mut self)`

**SideResponse**
- `.any(&self) -> bool`

**AtomLayoutResponse**
- `.custom_rects(&self) -> impl Iterator<Item = (Id, Rect)> + '_`
- `.empty(response: Response) -> Self`
- `.rect(&self, id: Id) -> Option<Rect>` — Use this together with crate::Atom::custom to add custom painting / child widgets.

## Painter — immediate drawing

**Painter**
- `.add(&self, shape: impl Into<Shape>) -> ShapeIdx` — It is up to the caller to make sure there is room for this.
- `.arrow(&self, origin: Pos2, vec: Vec2, stroke: impl Into<Stroke>)` — Show an arrow starting at origin and going in the direction of vec, with the length vec.length().
- `.circle(&self, center: Pos2, radius: f32, fill_color: impl Into<Color32>, stroke: impl Into<Stroke>,) -> ShapeIdx`
- `.circle_filled(&self, center: Pos2, radius: f32, fill_color: impl Into<Color32>,) -> ShapeIdx`
- `.circle_stroke(&self, center: Pos2, radius: f32, stroke: impl Into<Stroke>) -> ShapeIdx`
- `.clip_rect(&self) -> Rect` — Everything painted in this Painter will be clipped against this.
- `.ctx(&self) -> &Context` — Get a reference to the parent Context.
- `.debug_rect(&self, rect: Rect, color: Color32, text: impl ToString)`
- `.debug_text(&self, pos: Pos2, anchor: Align2, color: Color32, text: impl ToString,) -> Rect` — Text with a background.
- `.error(&self, pos: Pos2, text: impl std::fmt::Display) -> Rect`
- `.extend<I: IntoIterator<Item = Shape>>(&self, shapes: I)` — Add many shapes at once.
- `.fonts<R>(&self, reader: impl FnOnce(&FontsView<'_>) -> R) -> R` — Read-only access to the shared FontsView.
- `.fonts_mut<R>(&self, reader: impl FnOnce(&mut FontsView<'_>) -> R) -> R` — Read-write access to the shared FontsView.
- `.for_each_shape(&self, mut reader: impl FnMut(&ClippedShape))` — Access all shapes added this frame.
- `.galley(&self, pos: Pos2, galley: Arc<Galley>, fallback_color: Color32)` — Paint text that has already been laid out in a Galley.
- `.galley_with_override_text_color(&self, pos: Pos2, galley: Arc<Galley>, text_color: Color32,)` — Paint text that has already been laid out in a Galley.
- `.hline(&self, x: impl Into<Rangef>, y: f32, stroke: impl Into<Stroke>) -> ShapeIdx` — Paints a horizontal line.
- `.image(&self, texture_id: epaint::TextureId, rect: Rect, uv: Rect, tint: Color32,) -> ShapeIdx` — An image at the given position.
- `.is_visible(&self) -> bool` — If false, nothing you paint will show up.
- `.layer_id(&self) -> LayerId` — Where we paint
- `.layout(&self, text: String, font_id: FontId, color: crate::Color32, wrap_width: f32,) -> Arc<Galley>` — Will wrap text at the given width and line break at \n.
- `.layout_job(&self, layout_job: LayoutJob) -> Arc<Galley>` — Lay out this text layut job in a galley.
- `.layout_no_wrap(&self, text: String, font_id: FontId, color: crate::Color32,) -> Arc<Galley>` — Will line break at \n.
- `.line(&self, points: Vec<Pos2>, stroke: impl Into<PathStroke>) -> ShapeIdx` — Paints a line connecting the points.
- `.line_segment(&self, points: [Pos2; 2], stroke: impl Into<Stroke>) -> ShapeIdx` — Paints a line from the first point to the second.
- `.multiply_opacity(&mut self, opacity: f32)` — Like Self::set_opacity, but multiplies the given value with the current opacity.
- `.new(ctx: Context, layer_id: LayerId, clip_rect: Rect) -> Self` — Create a painter to a specific layer within a certain clip rectangle.
- `.opacity(&self) -> f32` — Read the current opacity of the underlying painter.
- `.pixels_per_point(&self) -> f32` — Number of physical pixels for each logical UI point.
- `.rect(&self, rect: Rect, corner_radius: impl Into<CornerRadius>, fill_color: impl Into<Color32>, stroke: impl Into<Stroke>, stroke_kind: StrokeKind,) -> ShapeIdx` — See also Self::rect_filled and Self::rect_stroke.
- `.rect_filled(&self, rect: Rect, corner_radius: impl Into<CornerRadius>, fill_color: impl Into<Color32>,) -> ShapeIdx`
- `.rect_stroke(&self, rect: Rect, corner_radius: impl Into<CornerRadius>, stroke: impl Into<Stroke>, stroke_kind: StrokeKind,) -> ShapeIdx`
- `.round_to_pixel_center(&self, point: f32) -> f32` — Useful for pixel-perfect rendering of lines that are one pixel wide (or any odd number of pixels).
- `.set(&self, idx: ShapeIdx, shape: impl Into<Shape>)` — Modify an existing Shape.
- `.set_clip_rect(&mut self, clip_rect: Rect)` — Everything painted in this Painter will be clipped against this.
- `.set_invisible(&mut self)` — If false, nothing added to the painter will be visible
- `.set_layer_id(&mut self, layer_id: LayerId)` — Redirect where you are painting.
- `.set_opacity(&mut self, opacity: f32)` — Set the opacity (alpha multiplier) of everything painted by this painter from this point forward.
- `.shrink_clip_rect(&mut self, new_clip_rect: Rect)` — Constrain the rectangle in which we can paint.
- `.text(&self, pos: Pos2, anchor: Align2, text: impl ToString, font_id: FontId, text_color: Color32,) -> Rect` — Lay out and paint some text.
- `.vline(&self, x: f32, y: impl Into<Rangef>, stroke: impl Into<Stroke>) -> ShapeIdx` — Paints a vertical line.
- `.with_clip_rect(&self, rect: Rect) -> Self` — Create a painter for a sub-region of this Painter.
- `.with_layer_id(mut self, layer_id: LayerId) -> Self` — Redirect where you are painting.

**Shape**
- `.circle_filled(center: Pos2, radius: f32, fill_color: impl Into<Color32>) -> Self`
- `.circle_stroke(center: Pos2, radius: f32, stroke: impl Into<Stroke>) -> Self`
- `.closed_line(points: Vec<Pos2>, stroke: impl Into<PathStroke>) -> Self` — A line that closes back to the start point again.
- `.convex_polygon(points: Vec<Pos2>, fill: impl Into<Color32>, stroke: impl Into<PathStroke>,) -> Self` — A convex polygon with a fill and optional stroke.
- `.dashed_line(path: &[Pos2], stroke: impl Into<Stroke>, dash_length: f32, gap_length: f32,) -> Vec<Self>` — Turn a line into dashes.
- `.dashed_line_many(points: &[Pos2], stroke: impl Into<Stroke>, dash_length: f32, gap_length: f32, shapes: &mut Vec<Self>,)` — Turn a line into dashes.
- `.dashed_line_many_with_offset(points: &[Pos2], stroke: impl Into<Stroke>, dash_lengths: &[f32], gap_lengths: &[f32], dash_offset: f32, shapes: &mut Vec<Self>,)` — Turn a line into dashes with different dash/gap lengths and a start offset.
- `.dashed_line_with_offset(path: &[Pos2], stroke: impl Into<Stroke>, dash_lengths: &[f32], gap_lengths: &[f32], dash_offset: f32,) -> Vec<Self>` — Turn a line into dashes with different dash/gap lengths and a start offset.
- `.dotted_line(path: &[Pos2], color: impl Into<Color32>, spacing: f32, radius: f32,) -> Vec<Self>` — Turn a line into equally spaced dots.
- `.ellipse_filled(center: Pos2, radius: Vec2, fill_color: impl Into<Color32>) -> Self`
- `.ellipse_stroke(center: Pos2, radius: Vec2, stroke: impl Into<Stroke>) -> Self`
- `.galley(pos: Pos2, galley: Arc<Galley>, fallback_color: Color32) -> Self` — Any uncolored parts of the Galley (using Color32::PLACEHOLDER) will be replaced with the given color.
- `.galley_with_override_text_color(pos: Pos2, galley: Arc<Galley>, text_color: Color32,) -> Self` — All text color in the Galley will be replaced with the given color.
- `.gradient_rect(rect: Rect, direction: Direction, [from, to]: [Color32; 2]) -> Self` — Paints a gradient rectangle that transitions from color_from to color_to along the given direction.
- `.hline(x: impl Into<Rangef>, y: f32, stroke: impl Into<Stroke>) -> Self` — A horizontal line.
- `.image(texture_id: TextureId, rect: Rect, uv: Rect, tint: Color32) -> Self` — An image at the given position.
- `.line(points: Vec<Pos2>, stroke: impl Into<PathStroke>) -> Self` — A line through many points.
- `.line_segment(points: [Pos2; 2], stroke: impl Into<Stroke>) -> Self` — A line between two points.
- `.mesh(mesh: impl Into<Arc<Mesh>>) -> Self`
- `.rect_filled(rect: Rect, corner_radius: impl Into<CornerRadius>, fill_color: impl Into<Color32>,) -> Self` — See also Self::rect_stroke.
- `.rect_stroke(rect: Rect, corner_radius: impl Into<CornerRadius>, stroke: impl Into<Stroke>, stroke_kind: StrokeKind,) -> Self` — See also Self::rect_filled.
- `.scale(&mut self, factor: f32)` — Scale the shape by factor, in-place.
- `.text(fonts: &mut FontsView<'_>, pos: Pos2, anchor: Align2, text: impl ToString, font_id: FontId, color: Color32,) -> Self`
- `.texture_id(&self) -> crate::TextureId`
- `.transform(&mut self, transform: TSTransform)` — Transform (move/scale) the shape in-place.
- `.translate(&mut self, delta: Vec2)` — Move the shape by delta, in-place.
- `.visual_bounding_rect(&self) -> Rect` — The visual bounding rectangle (includes stroke widths)
- `.vline(x: f32, y: impl Into<Rangef>, stroke: impl Into<Stroke>) -> Self` — A vertical line.

**PaintList**
- `.add(&mut self, clip_rect: Rect, shape: Shape) -> ShapeIdx` — Returns the index of the new Shape that can be used with PaintList::set.
- `.all_entries(&self) -> impl ExactSizeIterator<Item = &ClippedShape>` — Read-only access to all held shapes.
- `.extend<I: IntoIterator<Item = Shape>>(&mut self, clip_rect: Rect, shapes: I)`
- `.is_empty(&self) -> bool`
- `.mutate_shape(&mut self, idx: ShapeIdx, f: impl FnOnce(&mut ClippedShape))` — Mutate the shape at the given index, if any.
- `.next_idx(&self) -> ShapeIdx`
- `.reset_shape(&mut self, idx: ShapeIdx)` — Set the given shape to be empty (a Shape::Noop).
- `.set(&mut self, idx: ShapeIdx, clip_rect: Rect, shape: Shape)` — Modify an existing Shape.
- `.transform(&mut self, transform: TSTransform)` — Transform each Shape and clip rectangle by this much, in-place
- `.transform_range(&mut self, start: ShapeIdx, end: ShapeIdx, transform: TSTransform)` — Transform each Shape and clip rectangle in range by this much, in-place

**GraphicLayers**
- `.drain(&mut self, area_order: &[LayerId], to_global: &ahash::HashMap<LayerId, TSTransform>,) -> Vec<ClippedShape>`
- `.entry(&mut self, layer_id: LayerId) -> &mut PaintList` — Get or insert the PaintList for the given LayerId.
- `.get(&self, layer_id: LayerId) -> Option<&PaintList>` — Get the PaintList for the given LayerId.
- `.get_mut(&mut self, layer_id: LayerId) -> Option<&mut PaintList>` — Get the PaintList for the given LayerId.

**TextShape**
- `.new(pos: Pos2, galley: Arc<Galley>, fallback_color: Color32) -> Self` — The given fallback color will be used for any uncolored part of the galley (using Color32::PLACEHOLDER).
- `.transform(&mut self, transform: emath::TSTransform)` — Move the shape by this many points, in-place.
- `.visual_bounding_rect(&self) -> Rect` — The visual bounding rectangle
- `.with_angle(mut self, angle: f32) -> Self` — Set text rotation to angle radians clockwise.
- `.with_angle_and_anchor(mut self, angle: f32, anchor: Align2) -> Self` — Set the text rotation to the angle radians clockwise.
- `.with_opacity_factor(mut self, opacity_factor: f32) -> Self` — Render text with this opacity in gamma space
- `.with_override_text_color(mut self, override_text_color: Color32) -> Self` — Use the given color for the text, regardless of what color is already in the galley.
- `.with_underline(mut self, underline: Stroke) -> Self`

**PathShape**
- `.closed_line(points: Vec<Pos2>, stroke: impl Into<PathStroke>) -> Self` — A line that closes back to the start point again.
- `.convex_polygon(points: Vec<Pos2>, fill: impl Into<Color32>, stroke: impl Into<PathStroke>,) -> Self` — A convex polygon with a fill and optional stroke.
- `.line(points: Vec<Pos2>, stroke: impl Into<PathStroke>) -> Self` — A line through many points.
- `.visual_bounding_rect(&self) -> Rect` — The visual bounding rectangle (includes stroke width)

**RectShape**
- `.fill_texture_id(&self) -> TextureId` — The texture to use when painting this rectangle, if any.
- `.filled(rect: Rect, corner_radius: impl Into<CornerRadius>, fill_color: impl Into<Color32>,) -> Self`
- `.new(rect: Rect, corner_radius: impl Into<CornerRadius>, fill_color: impl Into<Color32>, stroke: impl Into<Stroke>, stroke_kind: StrokeKind,) -> Self` — See also Self::filled and Self::stroke.
- `.stroke(rect: Rect, corner_radius: impl Into<CornerRadius>, stroke: impl Into<Stroke>, stroke_kind: StrokeKind,) -> Self`
- `.visual_bounding_rect(&self) -> Rect` — The visual bounding rectangle (includes stroke width)
- `.with_angle(mut self, angle: f32) -> Self` — Set the rotation of the rectangle (in radians, clockwise).
- `.with_angle_and_pivot(mut self, angle: f32, pivot: Pos2) -> Self` — Set the rotation of the rectangle (in radians, clockwise) around a custom pivot point.
- `.with_blur_width(mut self, blur_width: f32) -> Self` — If larger than zero, the edges of the rectangle (for both fill and stroke) will be blurred.
- `.with_round_to_pixels(mut self, round_to_pixels: bool) -> Self` — Snap the rectangle to pixels?
- `.with_stroke_kind(mut self, stroke_kind: StrokeKind) -> Self` — Set if the stroke is on the inside, outside, or centered on the rectangle.
- `.with_texture(mut self, fill_texture_id: TextureId, uv: Rect) -> Self` — Set the texture to use when painting this rectangle, if any.

**CircleShape**
- `.filled(center: Pos2, radius: f32, fill_color: impl Into<Color32>) -> Self`
- `.stroke(center: Pos2, radius: f32, stroke: impl Into<Stroke>) -> Self`
- `.visual_bounding_rect(&self) -> Rect` — The visual bounding rectangle (includes stroke width)

**EllipseShape**
- `.filled(center: Pos2, radius: Vec2, fill_color: impl Into<Color32>) -> Self`
- `.stroke(center: Pos2, radius: Vec2, stroke: impl Into<Stroke>) -> Self`
- `.visual_bounding_rect(&self) -> Rect` — The visual bounding rectangle (includes stroke width)
- `.with_angle(mut self, angle: f32) -> Self` — Set the rotation of the ellipse (in radians, clockwise).
- `.with_angle_and_pivot(mut self, angle: f32, pivot: Pos2) -> Self` — Set the rotation of the ellipse (in radians, clockwise) around a custom pivot point.

**CubicBezierShape**
- `.find_cross_t(&self, epsilon: f32) -> Option<f32>` — Find out the t value for the point where the curve is intersected with the base line.
- `.flatten(&self, tolerance: Option<f32>) -> Vec<Pos2>` — find a set of points that approximate the cubic Bézier curve. the number of points is determined by the tolerance. the points may not be evenly distributed i...
- `.flatten_closed(&self, tolerance: Option<f32>, epsilon: Option<f32>) -> Vec<Vec<Pos2>>` — find a set of points that approximate the cubic Bézier curve. the number of points is determined by the tolerance. the points may not be evenly distributed i...
- `.for_each_flattened_with_t<F: FnMut(Pos2, f32)>(&self, tolerance: f32, callback: &mut F)` — Iterates through the curve invoking a callback at each point.
- `.from_points_stroke(points: [Pos2; 4], closed: bool, fill: Color32, stroke: impl Into<PathStroke>,) -> Self` — Creates a cubic Bézier curve based on 4 points and stroke.
- `.logical_bounding_rect(&self) -> Rect` — Logical bounding rectangle (ignoring stroke width)
- `.num_quadratics(&self, tolerance: f32) -> u32`
- `.sample(&self, t: f32) -> Pos2` — Calculate the point (x,y) at t based on the cubic Bézier curve equation. t is in [0.0,1.0] [Bézier Curve](https://en.wikipedia.org/wiki/B%C3%A9zier_curve#Cub...
- `.split_range(&self, t_range: Range<f32>) -> Self` — split the original cubic curve into a new one within a range.
- `.to_path_shapes(&self, tolerance: Option<f32>, epsilon: Option<f32>) -> Vec<PathShape>` — Convert the cubic Bézier curve to one or two PathShape's.
- `.transform(&self, transform: &RectTransform) -> Self` — Transform the curve with the given transform.
- `.visual_bounding_rect(&self) -> Rect` — The visual bounding rectangle (includes stroke width)

**QuadraticBezierShape**
- `.flatten(&self, tolerance: Option<f32>) -> Vec<Pos2>` — find a set of points that approximate the quadratic Bézier curve. the number of points is determined by the tolerance. the points may not be evenly distribut...
- `.for_each_flattened_with_t<F>(&self, tolerance: f32, callback: &mut F) where F: FnMut(Pos2, f32),` — Compute a flattened approximation of the curve, invoking a callback at each step.
- `.from_points_stroke(points: [Pos2; 3], closed: bool, fill: Color32, stroke: impl Into<PathStroke>,) -> Self` — Create a new quadratic Bézier shape based on the 3 points and stroke.
- `.logical_bounding_rect(&self) -> Rect` — Logical bounding rectangle (ignoring stroke width)
- `.sample(&self, t: f32) -> Pos2` — Calculate the point (x,y) at t based on the quadratic Bézier curve equation. t is in [0.0,1.0] [Bézier Curve](https://en.wikipedia.org/wiki/B%C3%A9zier_curve...
- `.to_path_shape(&self, tolerance: Option<f32>) -> PathShape` — Convert the quadratic Bézier curve to one PathShape.
- `.transform(&self, transform: &RectTransform) -> Self` — Transform the curve with the given transform.
- `.visual_bounding_rect(&self) -> Rect` — The visual bounding rectangle (includes stroke width)

**Mesh**
- `.add_colored_rect(&mut self, rect: Rect, color: Color32)` — Uniformly colored rectangle.
- `.add_rect_with_uv(&mut self, rect: Rect, uv: Rect, color: Color32)` — Rectangle with a texture and color.
- `.add_triangle(&mut self, a: u32, b: u32, c: u32)` — Add a triangle.
- `.append(&mut self, other: Self)` — Append all the indices and vertices of other to self.
- `.append_ref(&mut self, other: &Self)` — Append all the indices and vertices of other to self without taking ownership.
- `.bytes_used(&self) -> usize` — Returns the amount of memory used by the vertices and indices.
- `.calc_bounds(&self) -> Rect` — Calculate a bounding rectangle.
- `.clear(&mut self)` — Restore to default state, but without freeing memory.
- `.colored_vertex(&mut self, pos: Pos2, color: Color32)` — Add a colored vertex.
- `.is_empty(&self) -> bool`
- `.is_valid(&self) -> bool` — Are all indices within the bounds of the contained vertices?
- `.reserve_triangles(&mut self, additional_triangles: usize)` — Make room for this many additional triangles (will reserve 3x as many indices).
- `.reserve_vertices(&mut self, additional: usize)` — Make room for this many additional vertices.
- `.rotate(&mut self, rot: Rot2, origin: Pos2)` — Rotate by some angle about an origin, in-place.
- `.split_to_u16(self) -> Vec<Mesh16>` — This is for platforms that only support 16-bit index buffers.
- `.transform(&mut self, transform: TSTransform)` — Transform the mesh in-place with the given transform.
- `.translate(&mut self, delta: Vec2)` — Translate location by this much, in-place
- `.triangles(&self) -> impl Iterator<Item = [u32; 3]> + '_` — Iterate over the triangles of this mesh, returning vertex indices.
- `.with_texture(texture_id: TextureId) -> Self`

**Mesh16**
- `.is_valid(&self) -> bool` — Are all indices within the bounds of the contained vertices?

**Vertex**
- `.untextured(pos: Pos2, color: Color32) -> Self` — An untextured vertex

## Geometry — Vec2 / Pos2 / Rect / Rangef / Align

**Vec2**
- `.abs(self) -> Self`
- `.angle(self) -> f32` — Measures the angle of the vector.
- `.angled(angle: f32) -> Self` — Create a unit vector with the given CW angle (in radians). * An angle of zero gives the unit X axis. * An angle of 𝞃/4 = 90° gives the unit Y axis.
- `.any_nan(self) -> bool` — True if any member is NaN.
- `.ceil(self) -> Self`
- `.clamp(self, min: Self, max: Self) -> Self`
- `.dot(self, other: Self) -> f32` — The dot-product of two vectors.
- `.floor(self) -> Self`
- `.is_finite(self) -> bool` — True if all members are also finite.
- `.is_normalized(self) -> bool` — Checks if self has length 1.0 up to a precision of 1e-6.
- `.length(self) -> f32`
- `.length_sq(self) -> f32`
- `.max(self, other: Self) -> Self`
- `.max_elem(self) -> f32` — Returns the maximum of self.x and self.y.
- `.min(self, other: Self) -> Self`
- `.min_elem(self) -> f32` — Returns the minimum of self.x and self.y.
- `.new(x: f32, y: f32) -> Self`
- `.normalized(self) -> Self` — Safe normalize: returns zero if input is zero.
- `.rot90(self) -> Self` — Rotates the vector by 90°, i.e positive X to positive Y (clockwise in egui coordinates).
- `.round(self) -> Self`
- `.splat(v: f32) -> Self` — Set both x and y to the same value.
- `.to_pos2(self) -> crate::Pos2` — Treat this vector as a position. v.to_pos2() is equivalent to Pos2::default() + v.
- `.yx(self) -> Self` — Swizzle the axes.

**Vec2b**
- `.all(&self) -> bool` — Are both x and y true?
- `.and(&self, other: impl Into<Self>) -> Self`
- `.any(&self) -> bool`
- `.new(x: bool, y: bool) -> Self`
- `.or(&self, other: impl Into<Self>) -> Self`
- `.to_vec2(self) -> Vec2` — Convert to a float Vec2 where the components are 1.0 for true and 0.0 for false.

**Pos2**
- `.any_nan(self) -> bool` — True if any member is NaN.
- `.ceil(self) -> Self`
- `.clamp(self, min: Self, max: Self) -> Self`
- `.distance(self, other: Self) -> f32`
- `.distance_sq(self, other: Self) -> f32`
- `.floor(self) -> Self`
- `.is_finite(self) -> bool` — True if all members are also finite.
- `.lerp(&self, other: Self, t: f32) -> Self` — Linearly interpolate towards another point, so that 0.0 => self, 1.0 => other.
- `.max(self, other: Self) -> Self`
- `.min(self, other: Self) -> Self`
- `.new(x: f32, y: f32) -> Self`
- `.round(self) -> Self`
- `.to_vec2(self) -> Vec2` — The vector from origin to this position. p.to_vec2() is equivalent to p - Pos2::default().

**Rect**
- `.any_nan(self) -> bool` — True if any member is NaN.
- `.area(&self) -> f32` — This is never negative, and instead returns zero for negative rectangles.
- `.aspect_ratio(&self) -> f32` — Width / height
- `.bottom(&self) -> f32` — max.y
- `.bottom_mut(&mut self) -> &mut f32` — max.y
- `.bottom_up_range(&self) -> Rangef`
- `.center(&self) -> Pos2`
- `.center_bottom(&self) -> Pos2`
- `.center_top(&self) -> Pos2`
- `.clamp(&self, p: Pos2) -> Pos2` — Return the given points clamped to be inside the rectangle Panics if Self::is_negative.
- `.contains(&self, p: Pos2) -> bool`
- `.contains_rect(&self, other: Self) -> bool`
- `.distance_sq_to_pos(&self, pos: Pos2) -> f32` — The distance from the rect to the position, squared.
- `.distance_to_pos(&self, pos: Pos2) -> f32` — The distance from the rect to the position.
- `.everything_above(bottom_y: f32) -> Self` — A Rect that contains every point above a certain y coordinate
- `.everything_below(top_y: f32) -> Self` — A Rect that contains every point below a certain y coordinate
- `.everything_left_of(right_x: f32) -> Self` — A Rect that contains every point to the left of the given X coordinate.
- `.everything_right_of(left_x: f32) -> Self` — A Rect that contains every point to the right of the given X coordinate.
- `.expand(self, amnt: f32) -> Self` — Expand by this much in each direction, keeping the center
- `.expand2(self, amnt: Vec2) -> Self` — Expand by this much in each direction, keeping the center
- `.extend_with(&mut self, p: Pos2)`
- `.extend_with_x(&mut self, x: f32)` — Expand to include the given x coordinate
- `.extend_with_y(&mut self, y: f32)` — Expand to include the given y coordinate
- `.from_center_size(center: Pos2, size: Vec2) -> Self`
- `.from_min_max(min: Pos2, max: Pos2) -> Self`
- `.from_min_size(min: Pos2, size: Vec2) -> Self` — left-top corner plus a size (stretching right-down).
- `.from_points(points: &[Pos2]) -> Self` — Bounding-box around the points.
- `.from_pos(point: Pos2) -> Self` — A zero-sized rect at a specific point.
- `.from_two_pos(a: Pos2, b: Pos2) -> Self` — Returns the bounding rectangle of the two points.
- `.from_x_y_ranges(x_range: impl Into<Rangef>, y_range: impl Into<Rangef>) -> Self`
- `.height(&self) -> f32` — Note: this can be negative.
- `.intersect(self, other: Self) -> Self` — The intersection of two Rect, i.e. the area covered by both.
- `.intersects(self, other: Self) -> bool`
- `.intersects_ray(&self, o: Pos2, d: Vec2) -> bool` — Does this Rect intersect the given ray (where d is normalized)?
- `.intersects_ray_from_center(&self, d: Vec2) -> Pos2` — Where does a ray from the center intersect the rectangle?
- `.is_finite(&self) -> bool` — True if all members are also finite.
- `.is_negative(&self) -> bool` — width < 0 || height < 0
- `.is_positive(&self) -> bool` — width > 0 && height > 0
- `.left(&self) -> f32` — min.x
- `.left_bottom(&self) -> Pos2`
- `.left_center(&self) -> Pos2`
- `.left_mut(&mut self) -> &mut f32` — min.x
- `.left_top(&self) -> Pos2`
- `.lerp_inside(&self, t: impl Into<Vec2>) -> Pos2` — Linearly interpolate so that [0, 0] is Self::min and [1, 1] is Self::max.
- `.lerp_towards(&self, other: &Self, t: f32) -> Self` — Linearly self towards other rect.
- `.range_along(&self, axis: usize) -> Rangef` — The extent along the given axis: 0 for x, 1 for y.
- `.right(&self) -> f32` — max.x
- `.right_bottom(&self) -> Pos2`
- `.right_center(&self) -> Pos2`
- `.right_mut(&mut self) -> &mut f32` — max.x
- `.right_top(&self) -> Pos2`
- `.rotate_bb(self, rot: Rot2) -> Self` — Rotate the bounds (will expand the Rect)
- `.scale_from_center(self, scale_factor: f32) -> Self` — Scale up by this factor in each direction, keeping the center
- `.scale_from_center2(self, scale_factor: Vec2) -> Self` — Scale up by this factor in each direction, keeping the center
- `.set_bottom(&mut self, y: f32)` — max.y
- `.set_center(&mut self, center: Pos2)` — Keep size
- `.set_height(&mut self, h: f32)` — keep min
- `.set_left(&mut self, x: f32)` — min.x
- `.set_right(&mut self, x: f32)` — max.x
- `.set_top(&mut self, y: f32)` — min.y
- `.set_width(&mut self, w: f32)` — keep min
- `.shrink(self, amnt: f32) -> Self` — Shrink by this much in each direction, keeping the center
- `.shrink2(self, amnt: Vec2) -> Self` — Shrink by this much in each direction, keeping the center
- `.signed_distance_to_pos(&self, pos: Pos2) -> f32` — Signed distance to the edge of the box.
- `.size(&self) -> Vec2` — rect.size() == Vec2 { x: rect.width(), y: rect.height() }
- `.size_along(&self, axis: usize) -> f32` — The size along the given axis: 0 for x (width), 1 for y (height).
- `.split_left_right_at_fraction(&self, t: f32) -> (Self, Self)` — Split rectangle in left and right halves. t is expected to be in the (0,1) range.
- `.split_left_right_at_x(&self, split_x: f32) -> (Self, Self)` — Split rectangle in left and right halves at the given x coordinate.
- `.split_top_bottom_at_fraction(&self, t: f32) -> (Self, Self)` — Split rectangle in top and bottom halves. t is expected to be in the (0,1) range.
- `.split_top_bottom_at_y(&self, split_y: f32) -> (Self, Self)` — Split rectangle in top and bottom halves at the given y coordinate.
- `.square_proportions(&self) -> Vec2` — [2, 1] for wide screen, and [1, 2] for portrait, etc. At least one dimension = 1, the other >= 1 Returns the proportions required to letter-box a square view...
- `.top(&self) -> f32` — min.y
- `.top_mut(&mut self) -> &mut f32` — min.y
- `.translate(self, amnt: Vec2) -> Self`
- `.union(self, other: Self) -> Self` — The union of two bounding rectangle, i.e. the minimum Rect that contains both input rectangles.
- `.width(&self) -> f32` — Note: this can be negative.
- `.with_max_x(mut self, max_x: f32) -> Self`
- `.with_max_y(mut self, max_y: f32) -> Self`
- `.with_min_x(mut self, min_x: f32) -> Self`
- `.with_min_y(mut self, min_y: f32) -> Self`
- `.x_range(&self) -> Rangef`
- `.y_range(&self) -> Rangef`

**Rangef**
- `.as_positive(self) -> Self` — Flip min and max if needed, so that min <= max after.
- `.center(self) -> f32` — The center of the range
- `.clamp(self, x: f32) -> f32` — Equivalent to x.clamp(min, max)
- `.contains(self, x: f32) -> bool`
- `.expand(self, amnt: f32) -> Self` — Expand by this much on each side, keeping the center
- `.flip(self) -> Self` — Flip the min and the max
- `.intersection(self, other: Self) -> Self` — The overlap of two ranges, i.e. the range that is contained by both.
- `.intersects(self, other: Self) -> bool` — Do the two ranges intersect?
- `.new(min: f32, max: f32) -> Self`
- `.point(min_and_max: f32) -> Self`
- `.shrink(self, amnt: f32) -> Self` — Shrink by this much on each side, keeping the center
- `.span(self) -> f32` — The length of the range, i.e. max - min.

**Align**
- `.align_size_within_range(self, size: f32, range: impl Into<Rangef>) -> Rangef` — Returns a range of given size within a specified range.
- `.flip(self) -> Self` — Returns the inverse alignment.
- `.to_factor(self) -> f32` — Convert Min => 0.0, Center => 0.5 or Max => 1.0.
- `.to_sign(self) -> f32` — Convert Min => -1.0, Center => 0.0 or Max => 1.0.

**Align2**
- `.align_size_within_rect(self, size: Vec2, frame: Rect) -> Rect` — e.g. center a size within a given frame
- `.anchor_rect(self, rect: Rect) -> Rect` — Used e.g. to anchor a piece of text to a part of the rectangle.
- `.anchor_size(self, pos: Pos2, size: Vec2) -> Rect` — Use this anchor to position something around pos, e.g. Self::RIGHT_TOP means the right-top of the rect will end up at pos.
- `.flip(self) -> Self` — Flip on both axes e.g. TOP_LEFT -> BOTTOM_RIGHT
- `.flip_x(self) -> Self` — Flip on the x-axis e.g. TOP_LEFT -> TOP_RIGHT
- `.flip_y(self) -> Self` — Flip on the y-axis e.g. TOP_LEFT -> BOTTOM_LEFT
- `.pos_in_rect(self, frame: &Rect) -> Pos2` — Returns the point on the rect's frame or in the center of a rect according to the alignments of this object.
- `.to_sign(self) -> Vec2` — -1, 0, or +1 for each axis
- `.x(self) -> Align` — Returns an alignment by the X (horizontal) axis
- `.y(self) -> Align` — Returns an alignment by the Y (vertical) axis

**RectAlign**
- `.align_rect(&self, parent_rect: &Rect, size: Vec2, gap: f32) -> Rect` — Calculate the child rect based on a size and some optional gap.
- `.anchor(&self, parent_rect: &Rect, gap: f32) -> Pos2` — Calculator the anchor point for the child rect, based on the parent rect and an optional gap.
- `.child(&self) -> Align2` — Align in the child rect.
- `.find_best_align(values_to_try: impl Iterator<Item = Self>, content_rect: Rect, parent_rect: Rect, gap: f32, expected_size: Vec2,) -> Option<Self>` — Look for the first alternative RectAlign that allows the child rect to fit inside the content_rect.
- `.flip(self) -> Self` — Flip the alignment on both axes.
- `.flip_x(self) -> Self` — Flip the alignment on the x-axis.
- `.flip_y(self) -> Self` — Flip the alignment on the y-axis.
- `.from_align2(align: Align2) -> Self` — Convert an Align2 to an RectAlign, positioning the child rect inside the parent.
- `.gap_vector(&self) -> Vec2` — Returns a sign vector (-1, 0 or 1 in each direction) that can be used as an offset to the child rect, creating a gap between the rects while keeping the edge...
- `.outside(align: Align2) -> Self` — Position the child rect outside the parent rect.
- `.over_corner(align: Align2) -> Self` — The center of the child rect will be aligned to a corner of the parent rect.
- `.parent(&self) -> Align2` — Align in the parent rect.
- `.pivot_pos(&self, parent_rect: &Rect, gap: f32) -> (Align2, Pos2)` — Returns a Align2 and a Pos2 that you can e.g. use with Area::fixed_pos and Area::pivot to align an Area to some rect.
- `.symmetries(self) -> [Self; 3]` — Returns the 3 alternative RectAligns that are flipped in various ways, for use with RectAlign::find_best_align.

**RectTransform**
- `.from(&self) -> &Rect`
- `.from_to(from: Rect, to: Rect) -> Self`
- `.identity(from_and_to: Rect) -> Self`
- `.inverse(&self) -> Self`
- `.scale(&self) -> Vec2` — The scale factors.
- `.to(&self) -> &Rect`
- `.transform_pos(&self, pos: Pos2) -> Pos2` — Transforms the given coordinate in the from space to the to space.
- `.transform_pos_clamped(&self, pos: Pos2) -> Pos2` — Transforms the given coordinate in the from space to the to space, clamping if necessary.
- `.transform_rect(&self, rect: Rect) -> Rect` — Transforms the given rectangle in the in-space to a rectangle in the out-space.

**Rot2**
- `.angle(self) -> f32`
- `.from_angle(angle: f32) -> Self` — Angle is clockwise in radians.
- `.inverse(self) -> Self`
- `.is_finite(self) -> bool`
- `.length(self) -> f32` — The factor by which vectors will be scaled.
- `.length_squared(self) -> f32`
- `.normalized(self) -> Self`

**TSTransform**
- `.from_scaling(scaling: f32) -> Self`
- `.from_translation(translation: Vec2) -> Self`
- `.inverse(&self) -> Self` — Inverts the transform.
- `.is_valid(&self) -> bool` — Is this a valid, invertible transform?
- `.mul_pos(&self, pos: Pos2) -> Pos2` — Transforms the given coordinate.
- `.mul_rect(&self, rect: Rect) -> Rect` — Transforms the given rectangle.
- `.new(translation: Vec2, scaling: f32) -> Self` — Creates a new translation that first scales points around (0, 0), then translates them.

**Margin**
- `.bottomf(self) -> f32` — Bottom margin, as f32
- `.is_same(self) -> bool` — Are the margin on every side the same?
- `.left_top(self) -> Vec2`
- `.leftf(self) -> f32` — Left margin, as f32
- `.right_bottom(self) -> Vec2`
- `.rightf(self) -> f32` — Right margin, as f32
- `.same(margin: i8) -> Self` — The same margin on every side.
- `.sum(self) -> Vec2` — Total margins on both sides
- `.symmetric(x: i8, y: i8) -> Self` — Margins with the same size on opposing sides
- `.topf(self) -> f32` — Top margin, as f32

**MarginF32**
- `.is_same(&self) -> bool` — Are the margin on every side the same?
- `.left_top(&self) -> Vec2`
- `.right_bottom(&self) -> Vec2`
- `.same(margin: f32) -> Self` — The same margin on every side.
- `.sum(&self) -> Vec2` — Total margins on both sides
- `.symmetric(x: f32, y: f32) -> Self` — Margins with the same size on opposing sides

**CornerRadius**
- `.at_least(self, min: u8) -> Self` — Make sure each corner has a rounding of at least this.
- `.at_most(self, max: u8) -> Self` — Make sure each corner has a rounding of at most this.
- `.average(&self) -> f32` — Average rounding of the corners.
- `.is_same(self) -> bool` — Do all corners have the same rounding?
- `.same(radius: u8) -> Self` — Same rounding on all four corners.

**CornerRadiusF32**
- `.at_least(&self, min: f32) -> Self` — Make sure each corner has a rounding of at least this.
- `.at_most(&self, max: f32) -> Self` — Make sure each corner has a rounding of at most this.
- `.is_same(&self) -> bool` — Do all corners have the same rounding?
- `.same(radius: f32) -> Self` — Same rounding on all four corners.

## Color & Stroke

**Color32**
- `.a(&self) -> u8` — Alpha (opacity).
- `.additive(self) -> Self` — Returns an additive version of self
- `.b(&self) -> u8` — Blue component multiplied by alpha.
- `.blend(self, on_top: Self) -> Self` — Blend two colors in gamma space, so that self is behind the argument.
- `.from_additive_luminance(l: u8) -> Self` — Additive white.
- `.from_black_alpha(a: u8) -> Self` — Black with the given opacity.
- `.from_gray(l: u8) -> Self` — Opaque gray.
- `.from_hex(hex: &str) -> Result<Self, ParseHexColorError>` — Parses a color from a hex string.
- `.from_rgb(r: u8, g: u8, b: u8) -> Self` — From RGB with alpha of 255 (opaque).
- `.from_rgb_additive(r: u8, g: u8, b: u8) -> Self` — From RGB into an additive color (will make everything it blend with brighter).
- `.from_rgba_premultiplied(r: u8, g: u8, b: u8, a: u8) -> Self` — From sRGBA with premultiplied alpha.
- `.from_rgba_unmultiplied(r: u8, g: u8, b: u8, a: u8) -> Self` — From sRGBA with separate alpha.
- `.from_rgba_unmultiplied_const(r: u8, g: u8, b: u8, a: u8) -> Self` — Same as Self::from_rgba_unmultiplied, but can be used in a const context.
- `.from_white_alpha(a: u8) -> Self` — White with the given opacity.
- `.g(&self) -> u8` — Green component multiplied by alpha.
- `.gamma_multiply(self, factor: f32) -> Self` — Multiply with 0.5 to make color half as opaque, perceptually.
- `.gamma_multiply_u8(self, factor: u8) -> Self` — Multiply with 127 to make color half as opaque, perceptually.
- `.intensity(&self) -> f32` — Intensity of the color.
- `.is_additive(self) -> bool` — Is the alpha=0 ?
- `.is_opaque(&self) -> bool`
- `.lerp_to_gamma(&self, other: Self, t: f32) -> Self` — Lerp this color towards other by t in gamma space.
- `.linear_multiply(self, factor: f32) -> Self` — Multiply with 0.5 to make color half as opaque in linear space.
- `.r(&self) -> u8` — Red component multiplied by alpha.
- `.to_array(&self) -> [u8; 4]` — Premultiplied RGBA
- `.to_hex(&self) -> String` — Formats the color as a hex string.
- `.to_normalized_gamma_f32(self) -> [f32; 4]` — Converts to floating point values in the range 0-1 without any gamma space conversion.
- `.to_opaque(self) -> Self` — Returns an opaque version of self
- `.to_srgba_unmultiplied(&self) -> [u8; 4]` — Convert to a normal "unmultiplied" RGBA color (i.e. with separate alpha).
- `.to_tuple(&self) -> (u8, u8, u8, u8)` — Premultiplied RGBA

**Hsva**
- `.from_additive_rgb(rgb: [f32; 3]) -> Self`
- `.from_additive_srgb([r, g, b]: [u8; 3]) -> Self`
- `.from_rgb(rgb: [f32; 3]) -> Self`
- `.from_rgba_premultiplied(r: f32, g: f32, b: f32, a: f32) -> Self` — From linear RGBA with premultiplied alpha
- `.from_rgba_unmultiplied(r: f32, g: f32, b: f32, a: f32) -> Self` — From linear RGBA without premultiplied alpha
- `.from_srgb([r, g, b]: [u8; 3]) -> Self`
- `.from_srgba_premultiplied([r, g, b, a]: [u8; 4]) -> Self` — From sRGBA with premultiplied alpha
- `.from_srgba_unmultiplied([r, g, b, a]: [u8; 4]) -> Self` — From sRGBA without premultiplied alpha
- `.new(h: f32, s: f32, v: f32, a: f32) -> Self`
- `.to_opaque(self) -> Self`
- `.to_rgb(&self) -> [f32; 3]`
- `.to_rgba_premultiplied(&self) -> [f32; 4]`
- `.to_rgba_unmultiplied(&self) -> [f32; 4]` — To linear space rgba in 0-1 range.
- `.to_srgb(&self) -> [u8; 3]`
- `.to_srgba_premultiplied(&self) -> [u8; 4]`
- `.to_srgba_unmultiplied(&self) -> [u8; 4]` — To gamma-space 0-255.

**Rgba**
- `.a(&self) -> f32`
- `.additive(self) -> Self` — Return an additive version of this color (alpha = 0)
- `.b(&self) -> f32`
- `.blend(self, on_top: Self) -> Self` — Blend two colors in linear space, so that self is behind the argument.
- `.from_black_alpha(a: f32) -> Self` — Transparent black
- `.from_gray(l: f32) -> Self`
- `.from_luminance_alpha(l: f32, a: f32) -> Self`
- `.from_rgb(r: f32, g: f32, b: f32) -> Self`
- `.from_rgba_premultiplied(r: f32, g: f32, b: f32, a: f32) -> Self`
- `.from_rgba_unmultiplied(r: f32, g: f32, b: f32, a: f32) -> Self`
- `.from_srgba_premultiplied(r: u8, g: u8, b: u8, a: u8) -> Self`
- `.from_srgba_unmultiplied(r: u8, g: u8, b: u8, a: u8) -> Self`
- `.from_white_alpha(a: f32) -> Self` — Transparent white
- `.g(&self) -> f32`
- `.intensity(&self) -> f32` — How perceptually intense (bright) is the color?
- `.is_additive(self) -> bool` — Is the alpha=0 ?
- `.multiply(self, alpha: f32) -> Self` — Multiply with e.g. 0.5 to make us half transparent
- `.r(&self) -> f32`
- `.to_array(&self) -> [f32; 4]` — Premultiplied RGBA
- `.to_opaque(&self) -> Self` — Returns an opaque version of self
- `.to_rgba_unmultiplied(&self) -> [f32; 4]` — unmultiply the alpha
- `.to_srgba_unmultiplied(&self) -> [u8; 4]` — unmultiply the alpha
- `.to_tuple(&self) -> (f32, f32, f32, f32)` — Premultiplied RGBA

**HexColor**
- `.color(&self) -> Color32` — Retrieves the inner Color32
- `.from_str_without_hash(s: &str) -> Result<Self, ParseHexColorError>` — Parses a string as a hex color without the leading # character

**Stroke**
- `.is_empty(&self) -> bool` — True if width is zero or color is transparent
- `.new(width: f32, color: impl Into<Color32>) -> Self`
- `.round_center_to_pixel(&self, pixels_per_point: f32, coord: &mut f32)` — For vertical or horizontal lines: round the stroke center to produce a sharp, pixel-aligned line.

**PathStroke**
- `.inside(self) -> Self` — Set the stroke to be painted entirely inside of the shape
- `.is_empty(&self) -> bool` — True if width is zero or color is solid and transparent
- `.middle(self) -> Self` — Set the stroke to be painted right on the edge of the shape, half inside and half outside.
- `.new(width: f32, color: impl Into<Color32>) -> Self`
- `.new_uv(width: f32, callback: impl Fn(Rect, Pos2) -> Color32 + Send + Sync + 'static,) -> Self` — Create a new PathStroke with a UV function
- `.outside(self) -> Self` — Set the stroke to be painted entirely outside of the shape
- `.with_kind(self, kind: StrokeKind) -> Self`

**Shadow**
- `.as_shape(&self, rect: Rect, corner_radius: impl Into<CornerRadius>) -> RectShape` — The argument is the rectangle of the shadow caster.
- `.margin(&self) -> MarginF32` — How much larger than the parent rect are we in each direction?

## Style — Style/Spacing/Visuals theming

**Style**
- `.button_style(&self, classes: &Classes, state: WidgetState) -> ButtonStyle` — The dedicated button style.
- `.checkbox_style(&self, classes: &Classes, state: WidgetState) -> CheckboxStyle` — The dedicated checkbox style.
- `.interact(&self, response: &Response) -> &WidgetVisuals` — Use this style for interactive things.
- `.interact_selectable(&self, response: &Response, selected: bool) -> WidgetVisuals`
- `.label_style(&self, classes: &Classes, state: WidgetState) -> LabelStyle` — The dedicated label style.
- `.noninteractive(&self) -> &WidgetVisuals` — Style to use for non-interactive widgets.
- `.separator_style(&self, _classes: &Classes, _state: WidgetState) -> SeparatorStyle` — The dedicated separator style.
- `.text_styles(&self) -> Vec<TextStyle>` — All known text styles.
- `.ui(&mut self, ui: &mut crate::Ui)`
- `.widget_style(&self, _classes: &Classes, state: WidgetState) -> WidgetStyle` — The general widget style.

**Spacing**
- `.icon_rectangles(&self, rect: Rect) -> (Rect, Rect)` — Returns small icon rectangle and big icon rectangle
- `.ui(&mut self, ui: &mut crate::Ui)`

**Visuals**
- `.dark() -> Self` — Default dark theme.
- `.disable(&self, color: Color32) -> Color32` — Returns a "disabled" version of the given color.
- `.disabled_alpha(&self) -> f32` — Disabled widgets have their alpha modified by this.
- `.gray_out(&self, color: Color32) -> Color32` — Returns a "grayed out" version of the given color.
- `.light() -> Self` — Default light theme.
- `.noninteractive(&self) -> &WidgetVisuals`
- `.strong_text_color(&self) -> Color32`
- `.text_color(&self) -> Color32`
- `.text_edit_bg_color(&self) -> Color32` — The background color of crate::TextEdit.
- `.ui(&mut self, ui: &mut crate::Ui)`
- `.weak_text_color(&self) -> Color32`
- `.window_fill(&self) -> Color32` — Window background color.
- `.window_stroke(&self) -> Stroke`

**WidgetVisuals**
- `.text_color(&self) -> Color32`
- `.ui(&mut self, ui: &mut crate::Ui)`

**Widgets**
- `.dark() -> Self`
- `.light() -> Self`
- `.state(&self, state: WidgetState) -> &WidgetVisuals` — The widget visuals according to the state
- `.style(&self, response: &Response) -> &WidgetVisuals`
- `.ui(&mut self, ui: &mut crate::Ui)`

**ScrollStyle**
- `.allocated_width(&self) -> f32` — Width of a solid vertical scrollbar, or height of a horizontal scroll bar, when it is at its widest.
- `.details_ui(&mut self, ui: &mut Ui)`
- `.floating() -> Self` — No scroll bars until you hover the scroll area, at which time they appear faintly, and then expand when you hover the scroll bars.
- `.solid() -> Self` — Solid scroll bars that always use up space
- `.thin() -> Self` — Thin scroll bars that expand on hover
- `.ui(&mut self, ui: &mut Ui)`

**ScrollFadeStyle**
- `.ui(&mut self, ui: &mut Ui)`

**Theme**
- `.default_style(self) -> crate::Style` — Default style for this theme.
- `.default_visuals(self) -> crate::Visuals` — Default visuals for this theme.
- `.from_dark_mode(dark_mode: bool) -> Self` — Chooses between Self::Dark or Self::Light based on a boolean value.

**ThemePreference**
- `.radio_buttons(&mut self, ui: &mut crate::Ui)` — Show radio-buttons to switch between light mode, dark mode and following the system theme.

**StyleModifier**
- `.apply(&self, style: &mut Style)` — Apply the modification to the given Style.
- `.new(f: impl Fn(&mut Style) + Send + Sync + 'static) -> Self` — Create a new StyleModifier from a function.

## Layout & Placer

**Layout**
- `.align_size_within_rect(&self, size: Vec2, outer: Rect) -> Rect`
- `.bottom_up(halign: Align) -> Self` — Place elements vertically, bottom up.
- `.centered_and_justified(main_dir: Direction) -> Self` — For when you want to add a single widget to a layout, and that widget should use up all available space.
- `.cross_align(&self) -> Align`
- `.cross_justify(&self) -> bool`
- `.from_main_dir_and_cross_align(main_dir: Direction, cross_align: Align) -> Self`
- `.horizontal_align(&self) -> Align` — e.g. for when aligning text within a button.
- `.horizontal_justify(&self) -> bool`
- `.horizontal_placement(&self) -> Align` — e.g. for adjusting the placement of something. * in horizontal layout: left or right? * in vertical layout: same as Self::horizontal_align.
- `.is_horizontal(&self) -> bool`
- `.is_vertical(&self) -> bool`
- `.left_to_right(valign: Align) -> Self` — Place elements horizontally, left to right.
- `.main_dir(&self) -> Direction`
- `.main_wrap(&self) -> bool`
- `.prefer_right_to_left(&self) -> bool`
- `.right_to_left(valign: Align) -> Self` — Place elements horizontally, right to left.
- `.top_down(halign: Align) -> Self` — Place elements vertically, top to bottom.
- `.top_down_justified(halign: Align) -> Self` — Top-down layout justified so that buttons etc fill the full available width.
- `.vertical_align(&self) -> Align` — e.g. for when aligning text within a button.
- `.vertical_justify(&self) -> bool`
- `.with_cross_align(self, cross_align: Align) -> Self` — The alignment to use on the cross axis.
- `.with_cross_justify(self, cross_justify: bool) -> Self` — Justify widgets along the cross axis?
- `.with_main_align(self, main_align: Align) -> Self` — The alignment to use on the main axis.
- `.with_main_justify(self, main_justify: bool) -> Self` — Justify widgets on the main axis?
- `.with_main_wrap(self, main_wrap: bool) -> Self` — Wrap widgets when we overflow the main axis?

**UiBuilder**
- `.accessibility_parent(mut self, parent_id: Id) -> Self` — Set the accessibility parent for this Ui.
- `.closable(mut self) -> Self` — Make this Ui closable.
- `.disabled(mut self) -> Self` — Make the new Ui disabled, i.e. grayed-out and non-interactive.
- `.id(mut self, id: Id) -> Self` — Set an id of the new Ui that is independent of the parent Ui.
- `.id_salt(mut self, id_salt: impl AsIdSalt) -> Self` — Seed the child Ui with this id_salt, which will be mixed with the Ui::id of the parent.
- `.invisible(mut self) -> Self` — Make the contents invisible.
- `.layer_id(mut self, layer_id: LayerId) -> Self` — Show the Ui in a different LayerId from its parent.
- `.layout(mut self, layout: Layout) -> Self` — Override the layout.
- `.max_rect(mut self, max_rect: Rect) -> Self` — Set the max rectangle, within which widgets will go.
- `.new() -> Self`
- `.sense(mut self, sense: Sense) -> Self` — Set if you want sense clicks and/or drags.
- `.sizing_pass(mut self) -> Self` — Set to true in special cases where we do one frame where we size up the contents of the Ui, without actually showing it.
- `.style(mut self, style: impl Into<Arc<Style>>) -> Self` — Override the style.
- `.ui_stack_info(mut self, ui_stack_info: UiStackInfo) -> Self` — Provide some information about the new Ui being built.

**UiStack**
- `.bg_color(&self) -> Color32` — The background color of this crate::Ui.
- `.contained_in(&self, kind: UiKind) -> bool` — Check if this node is or is contained in a crate::Ui of a specific kind.
- `.frame(&self) -> &Frame`
- `.has_visible_frame(&self) -> bool` — This this crate::Ui a crate::Frame with a visible stroke?
- `.is_area_ui(&self) -> bool` — Is this crate::Ui an crate::Area?
- `.is_panel_ui(&self) -> bool` — Is this crate::Ui a panel?
- `.is_root_ui(&self) -> bool` — Is this a root crate::Ui, i.e. created with crate::Ui::new()?
- `.iter(&self) -> UiStackIterator<'_>` — Return an iterator that walks the stack from this node to the root.
- `.kind(&self) -> Option<UiKind>`
- `.tags(&self) -> &UiTags` — User tags.

**UiStackInfo**
- `.new(kind: UiKind) -> Self` — Create a new UiStackInfo with the given kind and an empty frame.
- `.with_frame(mut self, frame: Frame) -> Self`
- `.with_tag(mut self, key: impl Into<String>) -> Self` — Insert a tag with no value.
- `.with_tag_value(mut self, key: impl Into<String>, value: impl Any + Send + Sync + 'static,) -> Self` — Insert a tag with some value.

**UiKind**
- `.is_area(&self) -> bool` — Is this any kind of crate::Area?
- `.is_panel(&self) -> bool` — Is this any kind of panel?

**UiTags**
- `.contains(&self, key: &str) -> bool`
- `.get_any(&self, key: &str) -> Option<&Arc<dyn Any + Send + Sync + 'static>>` — Get the value of a tag.
- `.get_downcast<T: Any + Send + Sync + 'static>(&self, key: &str) -> Option<&T>` — Get the value of a tag.
- `.insert(&mut self, key: impl Into<String>, value: Option<Arc<dyn Any + Send + Sync + 'static>>,)`

**Direction**
- `.is_horizontal(self) -> bool`
- `.is_vertical(self) -> bool`

**Sides**
- `.extend(mut self) -> Self` — Extend the left and right sides to fill the available space.
- `.height(mut self, height: f32) -> Self` — The minimum height of the sides.
- `.new() -> Self`
- `.show<RetL, RetR>(self, ui: &mut Ui, add_left: impl FnOnce(&mut Ui) -> RetL, add_right: impl FnOnce(&mut Ui) -> RetR,) -> (RetL, RetR)`
- `.shrink_left(mut self) -> Self` — Try to shrink widgets on the left side.
- `.shrink_right(mut self) -> Self` — Try to shrink widgets on the right side.
- `.spacing(mut self, spacing: f32) -> Self` — The horizontal spacing between the left and right UIs.
- `.truncate(mut self) -> Self` — Truncate the text on the shrinking side.
- `.wrap(mut self) -> Self` — Wrap the text on the shrinking side.
- `.wrap_mode(mut self, wrap_mode: crate::TextWrapMode) -> Self` — The text wrap mode for the shrinking side.

## Memory & persistent state

**Memory**
- `.allows_interaction(&self, layer_id: LayerId) -> bool` — Does this layer allow interaction? Returns true if - the layer is not behind a modal layer - the Order allows interaction
- `.area_rect(&self, id: impl Into<Id>) -> Option<Rect>` — Obtain the previous rectangle of an area.
- `.areas(&self) -> &Areas` — Access memory of the Areas, such as Windows.
- `.areas_mut(&mut self) -> &mut Areas` — Access memory of the Areas, such as Windows.
- `.everything_is_visible(&self) -> bool` — If true, all windows, menus, tooltips, etc., will be visible at once.
- `.focused(&self) -> Option<Id>` — Which widget has keyboard focus?
- `.had_focus_last_frame(&self, id: Id) -> bool` — Check if the layer had focus last frame. returns true if the layer had focus last frame, but not this one.
- `.has_focus(&self, id: Id) -> bool` — Does this widget have keyboard focus?
- `.interested_in_focus(&mut self, id: Id, layer_id: LayerId)` — Register this widget as being interested in getting keyboard focus.
- `.interrupt_ime(&mut self)` — Interrupt the current IME composition, if any.
- `.is_above_modal_layer(&self, layer_id: LayerId) -> bool` — Returns true if - this layer is the top-most modal layer or above it - there is no modal layer
- `.layer_id_at(&self, pos: Pos2) -> Option<LayerId>` — Top-most layer at the given position.
- `.layer_ids(&self) -> impl ExactSizeIterator<Item = LayerId> + '_` — An iterator over all layers.
- `.move_focus(&mut self, direction: FocusDirection)` — Move keyboard focus in a specific direction.
- `.owns_ime_events(&self, id: Id) -> bool` — Check if the widget owns IME events.
- `.request_focus(&mut self, id: Id)` — Give keyboard focus to a specific widget.
- `.reset_areas(&mut self)` — Forget window positions, sizes etc. Can be used to auto-layout windows.
- `.set_everything_is_visible(&mut self, value: bool)` — If true, all windows, menus, tooltips etc are to be visible at once.
- `.set_focus_lock_filter(&mut self, id: Id, event_filter: EventFilter)` — Set an event filter for a widget.
- `.set_modal_layer(&mut self, layer_id: LayerId)` — Limit focus to widgets on the given layer and above.
- `.stop_text_input(&mut self)` — Stop editing the active TextEdit (if any).
- `.surrender_focus(&mut self, id: Id)` — Surrender keyboard focus for a specific widget.
- `.top_modal_layer(&self) -> Option<LayerId>` — Get the top modal layer (from the previous frame).

**IdTypeMap**
- `.clear(&mut self)`
- `.count<T: 'static>(&self) -> usize` — Count the number of values are stored with the given type.
- `.count_serialized(&self) -> usize` — Count how many values are stored but not yet deserialized.
- `.get_persisted<T: SerializableAny>(&mut self, id: Id) -> Option<T>` — Read a value, optionally deserializing it if available.
- `.get_persisted_mut_or<T: SerializableAny>(&mut self, id: Id, or_insert: T) -> &mut T`
- `.get_persisted_mut_or_default<T: SerializableAny + Default>(&mut self, id: Id) -> &mut T`
- `.get_persisted_mut_or_insert_with<T: SerializableAny>(&mut self, id: Id, insert_with: impl FnOnce() -> T,) -> &mut T`
- `.get_temp<T: 'static + Clone>(&self, id: Id) -> Option<T>` — Read a value without trying to deserialize a persisted value.
- `.get_temp_mut_or<T: 'static + Any + Clone + Send + Sync>(&mut self, id: Id, or_insert: T,) -> &mut T`
- `.get_temp_mut_or_default<T: 'static + Any + Clone + Send + Sync + Default>(&mut self, id: Id,) -> &mut T`
- `.get_temp_mut_or_insert_with<T: 'static + Any + Clone + Send + Sync>(&mut self, id: Id, insert_with: impl FnOnce() -> T,) -> &mut T`
- `.get_temp_raw(&self, raw: RawKey) -> Option<&(dyn Any + Send + Sync)>` — Gets a reference to a value for a given raw key.
- `.get_temp_raw_mut(&mut self, raw: RawKey) -> Option<&mut (dyn Any + Send + Sync)>` — Gets a mutable reference to a value for a given raw key.
- `.insert_persisted<T: SerializableAny>(&mut self, id: Id, value: T)` — Insert a value that will be persisted next time you start the app.
- `.insert_temp<T: 'static + Any + Clone + Send + Sync>(&mut self, id: Id, value: T,) -> RawKey` — Insert a value that will not be persisted.
- `.is_empty(&self) -> bool`
- `.len(&self) -> usize`
- `.max_bytes_per_type(&self) -> usize` — The maximum number of bytes that will be used to store the persisted state of a single widget type.
- `.remove<T: 'static>(&mut self, id: Id)` — Remove the state of this type and id.
- `.remove_by_type<T: 'static>(&mut self)` — Note all state of the given type.
- `.remove_temp<T: 'static + Default>(&mut self, id: Id) -> Option<T>` — Remove and fetch the state of this type and id.
- `.remove_temp_raw(&mut self, raw: RawKey) -> Option<Box<dyn Any + Send + Sync>>` — Remove a temporary value given a raw key.
- `.set_max_bytes_per_type(&mut self, max_bytes_per_type: usize)` — See Self::max_bytes_per_type.
- `.temp_keys(&self) -> impl Iterator<Item = RawKey>` — Returns all RawKeys to values in this map.

**Id**
- `.accesskit_id(&self) -> accesskit::NodeId`
- `.new(source: impl AsId) -> Self` — Generate a new root Id by hashing some source (e.g. a string or integer).
- `.short_debug_format(&self) -> String` — Short and readable summary
- `.value(&self) -> u64` — The inner value of the Id.
- `.with(self, salt: impl AsIdSalt) -> Self` — Generate a child Id by salting the parent Id with the given argument.

**IdSalt**
- `.new(source: impl AsIdSalt) -> Self` — Create a new IdSalt by hashing some source (e.g. a string or integer).
- `.value(&self) -> u64` — The inner value of the IdSalt.

**FixedCache**
- `.get(&self, key: &K) -> Option<&V>`
- `.set(&mut self, key: K, value: V)`

**AreaState**
- `.left_top_pos(&self) -> Pos2` — The left top positions of the area.
- `.load(ctx: &Context, id: Id) -> Option<Self>` — Load the state of an Area from memory.
- `.rect(&self) -> Rect` — Where the area is on screen.
- `.set_left_top_pos(&mut self, pos: Pos2)` — Move the left top positions of the area.

**PanelState**
- `.load(ctx: &Context, bar_id: Id) -> Option<Self>`
- `.size(&self) -> Vec2` — The _outer_ size of the panel (from previous frame), i.e. including the Frame margin & border.

**Areas**
- `.child_layers(&self, layer_id: LayerId) -> impl Iterator<Item = LayerId> + '_` — All the child layers of this layer.
- `.is_visible(&self, layer_id: &LayerId) -> bool`
- `.layer_id_at(&self, pos: Pos2, layer_to_global: &HashMap<LayerId, TSTransform>,) -> Option<LayerId>` — Top-most layer at the given position.
- `.move_to_top(&mut self, layer_id: LayerId)`
- `.parent_layer(&self, layer_id: LayerId) -> Option<LayerId>` — If this layer is the sublayer of another layer, return the parent.
- `.set_sublayer(&mut self, parent: LayerId, child: LayerId)` — Mark the child layer as a sublayer of parent.
- `.top_layer_id(&self, order: Order) -> Option<LayerId>`
- `.visible_last_frame(&self, layer_id: &LayerId) -> bool`
- `.visible_layer_ids(&self) -> ahash::HashSet<LayerId>`

**History**
- `.add(&mut self, now: f64, value: T)` — Values must be added with a monotonically increasing time, or at least not decreasing.
- `.average(&self) -> Option<T>`
- `.bandwidth(&self) -> Option<T>` — Average times rate.
- `.clear(&mut self)`
- `.duration(&self) -> f32` — Amount of time contained from start to end in this History.
- `.flush(&mut self, now: f64)` — Remove samples that are too old.
- `.is_empty(&self) -> bool`
- `.iter(&self) -> impl ExactSizeIterator<Item = (f64, T)> + '_` — (time, value) pairs Time difference between values can be zero, but never negative.
- `.latest(&self) -> Option<T>`
- `.latest_mut(&mut self) -> Option<&mut T>`
- `.len(&self) -> usize` — Current number of values kept in history
- `.max_age(&self) -> f32`
- `.max_len(&self) -> usize`
- `.mean_time_interval(&self) -> Option<f32>` — Mean time difference between values in this History.
- `.new(length_range: std::ops::Range<usize>, max_age: f32) -> Self` — Example:
- `.rate(&self) -> Option<f32>`
- `.sum(&self) -> T`
- `.total_count(&self) -> u64` — Total number of values seen.
- `.values(&self) -> impl ExactSizeIterator<Item = T> + '_`
- `.velocity(&self) -> Option<Vel>` — Calculate a smooth velocity (per second) over the entire time span.

**Undoer**
- `.add_undo(&mut self, current_state: &State)` — Add an undo point if, and only if, there has been a change since the latest undo point.
- `.feed_state(&mut self, current_time: f64, current_state: &State)` — Call this as often as you want (e.g. every frame) and Undoer will determine if a new undo point should be created.
- `.has_redo(&self, current_state: &State) -> bool`
- `.has_undo(&self, current_state: &State) -> bool` — Do we have an undo point different from the given state?
- `.is_in_flux(&self) -> bool` — Return true if the state is currently changing
- `.redo(&mut self, current_state: &State) -> Option<&State>`
- `.undo(&mut self, current_state: &State) -> Option<&State>`
- `.with_settings(settings: Settings) -> Self` — Create a new Undoer with the given Settings.

## Input — pointer, keyboard, touch

**InputState**
- `.accesskit_action_requests(&self, id: crate::Id, action: accesskit::Action,) -> impl Iterator<Item = &accesskit::ActionRequest>`
- `.aim_radius(&self) -> f32` — How imprecise do we expect the mouse/touch input to be? Returns imprecision in points.
- `.any_touches(&self) -> bool` — True if there currently are any fingers touching egui.
- `.begin_pass(mut self, mut new: RawInput, requested_immediate_repaint_prev_frame: bool, pixels_per_point: f32, options: InputOptions,) -> Self`
- `.consume_accesskit_action_requests(&mut self, id: crate::Id, mut consume: impl FnMut(&accesskit::ActionRequest) -> bool,)`
- `.consume_key(&mut self, modifiers: Modifiers, logical_key: Key) -> bool` — Check for a key press.
- `.consume_shortcut(&mut self, shortcut: &KeyboardShortcut) -> bool` — Check if the given shortcut has been pressed.
- `.content_rect(&self) -> Rect` — Returns the region of the screen that is safe for content rendering
- `.count_and_consume_key(&mut self, modifiers: Modifiers, logical_key: Key) -> usize` — Count presses of a key.
- `.filtered_events(&self, filter: &EventFilter) -> Vec<Event>` — Get all events that matches the given filter.
- `.has_accesskit_action_request(&self, id: crate::Id, action: accesskit::Action) -> bool`
- `.has_touch_screen(&self) -> bool` — True if we have ever received a touch event.
- `.is_scrolling(&self) -> bool` — True if there is an active scroll action that might scroll more when using Self::smooth_scroll_delta.
- `.key_down(&self, desired_key: Key) -> bool` — Is the given key currently held down?
- `.key_pressed(&self, desired_key: Key) -> bool` — Was the given key pressed this frame?
- `.key_released(&self, desired_key: Key) -> bool` — Was the given key released this frame?
- `.multi_touch(&self) -> Option<MultiTouchInfo>` — Returns details about the currently ongoing multi-touch gesture, if any.
- `.num_accesskit_action_requests(&self, id: crate::Id, action: accesskit::Action) -> usize`
- `.num_presses(&self, desired_key: Key) -> usize` — How many times was the given key pressed this frame?
- `.physical_pixel_size(&self) -> f32` — Size of a physical pixel in logical gui coordinates (points).
- `.pixels_per_point(&self) -> f32` — Also known as device pixel ratio, > 1 for high resolution screens.
- `.rotation_delta(&self) -> f32` — Rotation in radians this frame, measuring clockwise (e.g. from a rotation gesture).
- `.safe_area_insets(&self) -> SafeAreaInsets` — Get the safe area insets.
- `.smooth_scroll_delta(&self) -> Vec2` — How many points the user scrolled, smoothed over a few frames.
- `.time_since_last_scroll(&self) -> f32` — How long has it been (in seconds) since the last scroll event?
- `.translation_delta(&self) -> Vec2` — Panning translation in pixels this frame (e.g. from scrolling or a pan gesture)
- `.ui(&self, ui: &mut crate::Ui)`
- `.viewport(&self) -> &ViewportInfo` — Info about the active viewport
- `.viewport_rect(&self) -> Rect` — Returns the full area available to egui, including parts that might be partially covered, for example, by the OS status bar or notches (see Self::safe_area_i...
- `.zoom_delta(&self) -> f32` — Uniform zoom scale factor this frame (e.g. from ctrl-scroll or pinch gesture). * zoom = 1: no change * zoom < 1: pinch together * zoom > 1: pinch spread
- `.zoom_delta_2d(&self) -> Vec2` — 2D non-proportional zoom scale factor this frame (e.g. from ctrl-scroll or pinch gesture).

**PointerState**
- `.any_click(&self) -> bool` — Were there any type of click this frame?
- `.any_down(&self) -> bool` — Is any pointer button currently down?
- `.any_pressed(&self) -> bool` — Was any pointer button pressed (!down -> down) this frame?
- `.any_released(&self) -> bool` — Was any pointer button released (down -> !down) this frame?
- `.button_clicked(&self, button: PointerButton) -> bool` — Was the given pointer button given clicked this frame?
- `.button_double_clicked(&self, button: PointerButton) -> bool` — Was the button given double clicked this frame?
- `.button_down(&self, button: PointerButton) -> bool` — Is this button currently down?
- `.button_pressed(&self, button: PointerButton) -> bool` — Was the button given pressed this frame?
- `.button_released(&self, button: PointerButton) -> bool` — Was the button given released this frame?
- `.button_triple_clicked(&self, button: PointerButton) -> bool` — Was the button given triple clicked this frame?
- `.could_any_button_be_click(&self) -> bool` — If the pointer button is down, will it register as a click when released?
- `.delta(&self) -> Vec2` — How much the pointer moved compared to last frame, in points.
- `.direction(&self) -> Vec2` — Current direction of the pointer.
- `.has_pointer(&self) -> bool` — Do we have a pointer?
- `.hover_pos(&self) -> Option<Pos2>` — If it is a good idea to show a tooltip, where is pointer?
- `.interact_pos(&self) -> Option<Pos2>` — If you detect a click or drag and wants to know where it happened, use this.
- `.is_decidedly_dragging(&self) -> bool` — Just because the mouse is down doesn't mean we are dragging.
- `.is_moving(&self) -> bool` — Is the pointer currently moving? This is smoothed so a few frames of stillness is required before this returns false.
- `.is_moving_towards_rect(&self, rect: &Rect) -> bool` — Is the mouse moving in the direction of the given rect?
- `.is_still(&self) -> bool` — Is the pointer currently still? This is smoothed so a few frames of stillness is required before this returns true.
- `.latest_pos(&self) -> Option<Pos2>` — Latest reported pointer position.
- `.middle_down(&self) -> bool` — Is the middle button currently down?
- `.motion(&self) -> Option<Vec2>` — How much the mouse moved since the last frame, in unspecified units.
- `.press_origin(&self) -> Option<Pos2>` — Where did the current click/drag originate? None if no mouse button is down.
- `.press_start_time(&self) -> Option<f64>` — When did the current click/drag originate? None if no mouse button is down.
- `.primary_clicked(&self) -> bool` — Was the primary button clicked this frame?
- `.primary_down(&self) -> bool` — Is the primary button currently down?
- `.primary_pressed(&self) -> bool` — Was the primary button pressed this frame?
- `.primary_released(&self) -> bool` — Was the primary button released this frame?
- `.secondary_clicked(&self) -> bool` — Was the secondary button clicked this frame?
- `.secondary_down(&self) -> bool` — Is the secondary button currently down?
- `.secondary_pressed(&self) -> bool` — Was the secondary button pressed this frame?
- `.secondary_released(&self) -> bool` — Was the secondary button released this frame?
- `.time_since_last_click(&self) -> f32` — How long has it been (in seconds) since the pointer was clicked?
- `.time_since_last_movement(&self) -> f32` — How long has it been (in seconds) since the pointer was last moved?
- `.total_drag_delta(&self) -> Option<Vec2>` — How far has the pointer moved since the start of the drag (if any)?
- `.ui(&self, ui: &mut crate::Ui)`
- `.velocity(&self) -> Vec2` — Current velocity of pointer.

**PointerEvent**
- `.is_click(&self) -> bool`
- `.is_press(&self) -> bool`
- `.is_release(&self) -> bool`

**Modifiers**
- `.all(&self) -> bool`
- `.any(&self) -> bool`
- `.cmd_ctrl_matches(&self, pattern: Self) -> bool` — Checks only cmd/ctrl, not alt/shift.
- `.command_only(&self) -> bool` — true if only Self::ctrl or only Self::mac_cmd is pressed.
- `.contains(&self, query: Self) -> bool` — Whether another set of modifiers is contained in this set of modifiers with proper handling of Self::command.
- `.is_none(&self) -> bool`
- `.matches_any(&self, pattern: Self) -> bool` — Check if any of the modifiers match exactly.
- `.matches_exact(&self, pattern: Self) -> bool` — Check for equality but with proper handling of Self::command.
- `.matches_logically(&self, pattern: Self) -> bool` — Checks that the ctrl/cmd matches, and that the shift/alt of the argument is a subset of the pressed key (self).
- `.plus(self, rhs: Self) -> Self`
- `.shift_only(&self) -> bool` — Is shift the only pressed button?
- `.ui(&self, ui: &mut crate::Ui)`

**Key**
- `.from_name(key: &str) -> Option<Self>` — Converts "A" to Key::A, Space to Key::Space, etc.
- `.name(self) -> &'static str` — Human-readable English name.
- `.symbol_or_name(self) -> &'static str` — Emoji or name representing the key

**KeyboardShortcut**
- `.format(&self, names: &ModifierNames<'_>, is_mac: bool) -> String`
- `.new(modifiers: Modifiers, logical_key: Key) -> Self`

**TouchState**
- `.any_touches(&self) -> bool` — Are there currently any fingers touching the surface?
- `.begin_pass(&mut self, time: f64, new: &RawInput, pointer_pos: Option<Pos2>)`
- `.info(&self) -> Option<MultiTouchInfo>`
- `.new(device_id: TouchDeviceId) -> Self`
- `.ui(&self, ui: &mut crate::Ui)`

**WheelState**
- `.after_events(&mut self, time: f64, dt: f32)`
- `.is_scrolling(&self) -> bool` — True if there is an active scroll action that might scroll more when using Self::smooth_wheel_delta.
- `.on_wheel_event(&mut self, viewport_rect: Rect, options: &InputOptions, time: f64, unit: MouseWheelUnit, delta: Vec2, phase: TouchPhase, latest_modifiers: Modifiers,)`
- `.ui(&self, ui: &mut crate::Ui)`

**RawInput**
- `.append(&mut self, newer: Self)` — Add on new input.
- `.take(&mut self) -> Self` — Helper: move volatile (deltas and events), clone the rest.
- `.ui(&self, ui: &mut crate::Ui)`
- `.viewport(&self) -> &ViewportInfo` — Info about the active viewport

**Click**
- `.is_double(&self) -> bool`
- `.is_triple(&self) -> bool`

**EventFilter**
- `.matches(&self, event: &Event) -> bool`

## Sense & interaction internals

**Sense**
- `.click() -> Self` — Sense clicks and hover, but not drags, and make the widget focusable.
- `.click_and_drag() -> Self` — Sense both clicks, drags and hover (e.g. a slider or window), and make the widget focusable.
- `.drag() -> Self` — Sense drags and hover, but not clicks.
- `.focusable_noninteractive() -> Self` — Senses no clicks or drags, but can be focused with the keyboard.
- `.hover() -> Self` — Senses no clicks or drags.
- `.interactive(&self) -> bool` — Returns true if we sense either clicks or drags.
- `.is_focusable(&self) -> bool`
- `.senses_click(&self) -> bool`
- `.senses_drag(&self) -> bool`

**InteractionSnapshot**
- `.ui(&self, ui: &mut crate::Ui)`

**InteractionState**
- `.is_using_pointer(&self) -> bool` — Are we currently clicking or dragging an egui widget?

**Interaction**
- `.ui(&mut self, ui: &mut crate::Ui)`

**Focus**
- `.focused(&self) -> Option<Id>` — Which widget currently has keyboard focus?

**FocusWidget**
- `.new(id: Id) -> Self`

**PossibleInteractions**
- `.resizable(&self) -> bool`

**DragAndDrop**
- `.clear_payload(ctx: &Context)` — Clears the payload, setting it to None.
- `.has_any_payload(ctx: &Context) -> bool` — Are we carrying a payload?
- `.has_payload_of_type<Payload>(ctx: &Context) -> bool where Payload: Any + Send + Sync,` — Are we carrying a payload of the given type?
- `.payload<Payload>(ctx: &Context) -> Option<Arc<Payload>> where Payload: Any + Send + Sync,` — Retrieve the payload, if any.
- `.set_payload<Payload>(ctx: &Context, payload: Payload) where Payload: Any + Send + Sync,` — Set a drag-and-drop payload.
- `.take_payload<Payload>(ctx: &Context) -> Option<Arc<Payload>> where Payload: Any + Send + Sync,` — Retrieve and clear the payload, if any.

**DragScroll**
- `.enabled(self, ctx: &Context) -> bool` — Whether drag-to-scroll is currently active.

**WidgetRect**
- `.transform(self, transform: emath::TSTransform) -> Self`

**WidgetRects**
- `.clear(&mut self)` — Clear the contents while retaining allocated memory.
- `.contains(&self, id: Id) -> bool`
- `.get(&self, id: Id) -> Option<&WidgetRect>`
- `.get_layer(&self, layer_id: LayerId) -> impl Iterator<Item = &WidgetRect> + '_` — All widgets in this layer, sorted back-to-front.
- `.info(&self, id: Id) -> Option<&WidgetInfo>`
- `.insert(&mut self, layer_id: LayerId, widget_rect: WidgetRect, options: InteractOptions)` — Insert the given widget rect in the given layer.
- `.layer_ids(&self) -> impl ExactSizeIterator<Item = LayerId> + '_` — All known layers with widgets.
- `.layers(&self) -> impl Iterator<Item = (&LayerId, &[WidgetRect])> + '_`
- `.order(&self, id: Id) -> Option<(LayerId, usize)>` — In which layer, and in which order in that layer?
- `.set_info(&mut self, id: Id, info: WidgetInfo)`

**WidgetInfo**
- `.description(&self) -> String` — This can be used by a text-to-speech system to describe the widget.
- `.drag_value(enabled: bool, value: f64) -> Self`
- `.labeled(typ: WidgetType, enabled: bool, label: impl ToString) -> Self`
- `.new(typ: WidgetType) -> Self`
- `.selected(typ: WidgetType, enabled: bool, selected: bool, label: impl ToString) -> Self` — checkboxes, radio-buttons etc
- `.slider(enabled: bool, value: f64, label: impl ToString) -> Self`
- `.text_edit(enabled: bool, prev_text_value: impl ToString, text_value: impl ToString, hint_text: impl ToString,) -> Self`
- `.text_selection_changed(enabled: bool, text_selection: Range<CharIndex>, current_text_value: impl ToString,) -> Self`

## Containers — Window / Area / Panel

**Window**
- `.anchor(mut self, align: Align2, offset: impl Into<Vec2>) -> Self` — Set anchor and distance.
- `.auto_sized(mut self) -> Self` — Not resizable, just takes the size of its contents.
- `.collapsible(mut self, collapsible: bool) -> Self` — Can the window be collapsed by clicking on its title?
- `.constrain(mut self, constrain: bool) -> Self` — Constrains this window to Context::content_rect.
- `.constrain_to(mut self, constrain_rect: Rect) -> Self` — Constrain the movement of the window to the given rectangle.
- `.current_pos(mut self, current_pos: impl Into<Pos2>) -> Self` — Set current position of the window.
- `.default_height(mut self, default_height: f32) -> Self` — Set initial height of the window.
- `.default_open(mut self, default_open: bool) -> Self` — Set initial collapsed state of the window
- `.default_pos(mut self, default_pos: impl Into<Pos2>) -> Self` — Set initial position of the window.
- `.default_rect(self, rect: Rect) -> Self` — Set initial position and size of the window.
- `.default_size(mut self, default_size: impl Into<Vec2>) -> Self` — Set initial size of the window.
- `.default_width(mut self, default_width: f32) -> Self` — Set initial width of the window.
- `.drag_area(mut self, drag_area: WindowDrag) -> Self` — Where the user can grab the window to move it.
- `.drag_to_scroll(mut self, drag_to_scroll: DragScroll) -> Self` — Controls scrolling the window by dragging the contents with the pointer.
- `.enabled(mut self, enabled: bool) -> Self` — If false the window will be grayed out and non-interactive.
- `.fade_in(mut self, fade_in: bool) -> Self` — If true, quickly fade in the Window when it first appears.
- `.fade_out(mut self, fade_out: bool) -> Self` — If true, quickly fade out the Window when it closes.
- `.fixed_pos(mut self, pos: impl Into<Pos2>) -> Self` — Sets the window position and prevents it from being dragged around.
- `.fixed_rect(self, rect: Rect) -> Self` — Sets the window pos and size and prevents it from being moved and resized by dragging its edges.
- `.fixed_size(mut self, size: impl Into<Vec2>) -> Self` — Sets the window size and prevents it from being resized by dragging its edges.
- `.frame(mut self, frame: Frame) -> Self` — Change the background color, margins, etc.
- `.from_viewport(id: ViewportId, viewport: ViewportBuilder) -> Self` — Construct a Window that follows the given viewport.
- `.hscroll(mut self, hscroll: bool) -> Self` — Enable/disable horizontal scrolling. false by default.
- `.id(mut self, id: Id) -> Self` — Assign a unique id to the Window.
- `.interactable(mut self, interactable: bool) -> Self` — If false, clicks goes straight through to what is behind us.
- `.max_height(mut self, max_height: f32) -> Self` — Set maximum height of the window.
- `.max_size(mut self, max_size: impl Into<Vec2>) -> Self` — Set maximum size of the window, equivalent to calling both max_width and max_height.
- `.max_width(mut self, max_width: f32) -> Self` — Set maximum width of the window.
- `.min_height(mut self, min_height: f32) -> Self` — Set minimum height of the window.
- `.min_size(mut self, min_size: impl Into<Vec2>) -> Self` — Set minimum size of the window, equivalent to calling both min_width and min_height.
- `.min_width(mut self, min_width: f32) -> Self` — Set minimum width of the window.
- `.movable(mut self, movable: bool) -> Self` — If false the window will be immovable.
- `.mutate(mut self, mutate: impl Fn(&mut Self)) -> Self` — Usage: Window::new(…).mutate(|w| w.resize = w.resize.auto_expand_width(true))
- `.new(title: impl IntoAtoms<'a>) -> Self` — The window title is used as a unique Id and must be unique, and should not change.
- `.open(mut self, open: &'a mut bool) -> Self` — Call this to add a close-button to the window title bar.
- `.order(mut self, order: Order) -> Self` — order(Order::Foreground) for a Window that should always be on top
- `.pivot(mut self, pivot: Align2) -> Self` — Where the "root" of the window is.
- `.resizable(mut self, resizable: impl Into<Vec2b>) -> Self` — Can the user resize the window by dragging its edges?
- `.resize(mut self, mutate: impl Fn(Resize) -> Resize) -> Self` — Usage: Window::new(…).resize(|r| r.auto_expand_width(true))
- `.scroll(mut self, scroll: impl Into<Vec2b>) -> Self` — Enable/disable horizontal/vertical scrolling. false by default.
- `.scroll_bar_visibility(mut self, visibility: ScrollBarVisibility) -> Self` — Sets the ScrollBarVisibility of the window.
- `.show<R>(self, ctx: &Context, add_contents: impl FnOnce(&mut Ui) -> R,) -> Option<InnerResponse<Option<R>>>` — Returns None if the window is not open (if Window::open was called with &mut false).
- `.title_bar(mut self, title_bar: bool) -> Self` — Show title bar on top of the window? If false, the window will not be collapsible nor have a close-button.
- `.vscroll(mut self, vscroll: bool) -> Self` — Enable/disable vertical scrolling. false by default.

**Area**
- `.anchor(mut self, align: Align2, offset: impl Into<Vec2>) -> Self` — Set anchor and distance.
- `.constrain(mut self, constrain: bool) -> Self` — Constrains this area to Context::content_rect?
- `.constrain_to(mut self, constrain_rect: Rect) -> Self` — Constrain the movement of the window to the given rectangle.
- `.current_pos(mut self, current_pos: impl Into<Pos2>) -> Self` — Positions the window but you can still move it.
- `.default_height(mut self, default_height: f32) -> Self` — See Self::default_size.
- `.default_pos(mut self, default_pos: impl Into<Pos2>) -> Self`
- `.default_size(mut self, default_size: impl Into<Vec2>) -> Self` — The size used for the Ui::max_rect the first frame.
- `.default_width(mut self, default_width: f32) -> Self` — See Self::default_size.
- `.enabled(mut self, enabled: bool) -> Self` — If false, no content responds to click and widgets will be shown grayed out.
- `.fade_in(mut self, fade_in: bool) -> Self` — If true, quickly fade in the area.
- `.fixed_pos(mut self, fixed_pos: impl Into<Pos2>) -> Self` — Positions the window and prevents it from being moved
- `.id(mut self, id: Id) -> Self` — Let's you change the id that you assigned in Self::new.
- `.info(mut self, info: UiStackInfo) -> Self` — Set the UiStackInfo of the area's Ui.
- `.interactable(mut self, interactable: bool) -> Self` — If false, clicks goes straight through to what is behind us.
- `.is_enabled(&self) -> bool`
- `.is_movable(&self) -> bool`
- `.kind(mut self, kind: UiKind) -> Self` — Change the UiKind of the arena.
- `.layer(&self) -> LayerId`
- `.layout(mut self, layout: Layout) -> Self` — Set the layout for the child Ui.
- `.movable(mut self, movable: bool) -> Self` — Moveable by dragging the area?
- `.new(id: Id) -> Self` — The id must be globally unique.
- `.order(mut self, order: Order) -> Self` — order(Order::Foreground) for an Area that should always be on top
- `.pivot(mut self, pivot: Align2) -> Self` — Where the "root" of the area is.
- `.sense(mut self, sense: Sense) -> Self` — Explicitly set a sense.
- `.show<R>(self, ctx: &Context, add_contents: impl FnOnce(&mut Ui) -> R,) -> InnerResponse<R>`
- `.sizing_pass(mut self, resize: bool) -> Self` — While true, a sizing pass will be done.

**Panel**
- `.bottom(id: impl Into<Id>) -> Self` — Create a bottom panel.
- `.default_size(mut self, default_size: f32) -> Self` — The initial wrapping width of the Panel, including margins.
- `.exact_size(mut self, size: f32) -> Self` — Enforce this exact size, including margins.
- `.frame(mut self, frame: Frame) -> Self` — Change the background color, margins, etc.
- `.left(id: impl Into<Id>) -> Self` — Create a left panel.
- `.max_size(mut self, max_size: f32) -> Self` — Maximum size of the panel, including margins.
- `.min_size(mut self, min_size: f32) -> Self` — Minimum size of the panel, including margins.
- `.resizable(mut self, resizable: bool) -> Self` — Can panel be resized by dragging the edge of it?
- `.right(id: impl Into<Id>) -> Self` — Create a right panel.
- `.show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>` — Show the panel inside a Ui.
- `.show_animated_between_inside<R>(ui: &mut Ui, is_expanded: bool, collapsed_panel: Self, expanded_panel: Self, add_contents: impl FnOnce(&mut Ui, f32) -> R,) -> InnerResponse<R>` — Renamed to Self::show_switched.
- `.show_animated_inside<R>(self, ui: &mut Ui, mut is_expanded: bool, add_contents: impl FnOnce(&mut Ui) -> R,) -> Option<InnerResponse<R>>` — Renamed to Self::show_collapsible.
- `.show_collapsible<R>(self, ui: &mut Ui, is_expanded: &mut bool, add_contents: impl FnOnce(&mut Ui) -> R,) -> Option<InnerResponse<R>>` — Show the panel if *is_expanded is true, otherwise hide it, with a slide animation in between.
- `.show_inside<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R,) -> InnerResponse<R>` — Renamed to Self::show.
- `.show_separator_line(mut self, show_separator_line: bool) -> Self` — Show a separator line, even when not interacting with it?
- `.show_switched<R>(ui: &mut Ui, is_expanded: &mut bool, collapsed_panel: Self, expanded_panel: Self, add_contents: impl FnOnce(&mut Ui, bool) -> R,) -> InnerResponse<R>` — Show either a collapsed or expanded panel, with a nice slide animation between.
- `.size_range(mut self, size_range: impl Into<Rangef>) -> Self` — The allowable size range for the panel, including margins.
- `.top(id: impl Into<Id>) -> Self` — Create a top panel.

**CentralPanel**
- `.default_margins() -> Self` — A central panel with a background color and some inner margins
- `.frame(mut self, frame: Frame) -> Self` — Change the background color, margins, etc.
- `.no_frame() -> Self` — A central panel with no margin or background color
- `.show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>` — Show the panel inside a Ui.
- `.show_inside<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R,) -> InnerResponse<R>` — Renamed to Self::show.

**Resize**
- `.auto_sized(self) -> Self` — Not manually resizable, just takes the size of its contents.
- `.default_height(mut self, height: f32) -> Self` — Preferred / suggested height.
- `.default_size(mut self, default_size: impl Into<Vec2>) -> Self`
- `.default_width(mut self, width: f32) -> Self` — Preferred / suggested width.
- `.fixed_size(mut self, size: impl Into<Vec2>) -> Self`
- `.id(mut self, id: Id) -> Self` — Assign an explicit and globally unique id.
- `.id_salt(mut self, id_salt: impl AsIdSalt) -> Self` — A source for the unique Id, e.g. .id_salt("second_resize_area") or .id_salt(loop_index).
- `.is_resizable(&self) -> Vec2b`
- `.max_height(mut self, max_height: f32) -> Self` — Won't expand to larger than this
- `.max_size(mut self, max_size: impl Into<Vec2>) -> Self` — Won't expand to larger than this
- `.max_width(mut self, max_width: f32) -> Self` — Won't expand to larger than this
- `.min_height(mut self, min_height: f32) -> Self` — Won't shrink to smaller than this
- `.min_size(mut self, min_size: impl Into<Vec2>) -> Self` — Won't shrink to smaller than this
- `.min_width(mut self, min_width: f32) -> Self` — Won't shrink to smaller than this
- `.resizable(mut self, resizable: impl Into<Vec2b>) -> Self` — Can you resize it with the mouse?
- `.show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R`
- `.with_stroke(mut self, with_stroke: bool) -> Self`

**ResizeInteraction**
- `.any_dragged(&self) -> bool`
- `.any_hovered(&self) -> bool`
- `.set_cursor(&self, ctx: &Context)`

## ScrollArea & Grid

**ScrollArea**
- `.animated(mut self, animated: bool) -> Self` — Should the scroll area animate scroll_to_* functions?
- `.auto_shrink(mut self, auto_shrink: impl Into<Vec2b>) -> Self` — For each axis, should the containing area shrink if the content is small?
- `.both() -> Self` — Create a bi-directional (horizontal and vertical) scroll area.
- `.content_margin(mut self, margin: impl Into<Margin>) -> Self` — Extra margin added around the contents.
- `.horizontal() -> Self` — Create a horizontal scroll area.
- `.horizontal_scroll_offset(mut self, offset: f32) -> Self` — Set the horizontal scroll offset position.
- `.hscroll(mut self, hscroll: bool) -> Self` — Turn on/off scrolling on the horizontal axis.
- `.id_salt(mut self, id_salt: impl AsIdSalt) -> Self` — A source for the unique Id, e.g. .id_salt("second_scroll_area") or .id_salt(loop_index).
- `.max_height(mut self, max_height: f32) -> Self` — The maximum height of the outer frame of the scroll area.
- `.max_width(mut self, max_width: f32) -> Self` — The maximum width of the outer frame of the scroll area.
- `.min_scrolled_height(mut self, min_scrolled_height: f32) -> Self` — The minimum height of a vertical scroll area which requires scroll bars.
- `.min_scrolled_width(mut self, min_scrolled_width: f32) -> Self` — The minimum width of a horizontal scroll area which requires scroll bars.
- `.neither() -> Self` — Create a scroll area where both direction of scrolling is disabled.
- `.new(direction_enabled: impl Into<Vec2b>) -> Self` — Create a scroll area where you decide which axis has scrolling enabled.
- `.on_drag_cursor(mut self, cursor: CursorIcon) -> Self` — Set the cursor used when the ScrollArea is being dragged.
- `.on_hover_cursor(mut self, cursor: CursorIcon) -> Self` — Set the cursor used when the mouse pointer is hovering over the ScrollArea.
- `.scroll(mut self, direction_enabled: impl Into<Vec2b>) -> Self` — Turn on/off scrolling on the horizontal/vertical axes.
- `.scroll_bar_rect(mut self, scroll_bar_rect: Rect) -> Self` — Specify within which screen-space rectangle to show the scroll bars.
- `.scroll_bar_visibility(mut self, scroll_bar_visibility: ScrollBarVisibility) -> Self` — Set the visibility of both horizontal and vertical scroll bars.
- `.scroll_offset(mut self, offset: Vec2) -> Self` — Set the horizontal and vertical scroll offset position.
- `.scroll_source(mut self, scroll_source: ScrollSource) -> Self` — Control the scrolling behavior.
- `.show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R,) -> ScrollAreaOutput<R>` — Show the ScrollArea, and add the contents to the viewport.
- `.show_rows<R>(self, ui: &mut Ui, row_height_sans_spacing: f32, total_rows: usize, add_contents: impl FnOnce(&mut Ui, std::ops::Range<usize>) -> R,) -> ScrollAreaOutput<R>` — Efficiently show only the visible part of a large number of rows.
- `.show_viewport<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui, Rect) -> R,) -> ScrollAreaOutput<R>` — This can be used to only paint the visible part of the contents.
- `.stick_to_bottom(mut self, stick: bool) -> Self` — The scroll handle will stick to the bottom position even while the content size changes dynamically.
- `.stick_to_right(mut self, stick: bool) -> Self` — The scroll handle will stick to the rightmost position even while the content size changes dynamically.
- `.vertical() -> Self` — Create a vertical scroll area.
- `.vertical_scroll_offset(mut self, offset: f32) -> Self` — Set the vertical scroll offset position.
- `.vscroll(mut self, vscroll: bool) -> Self` — Turn on/off scrolling on the vertical axis.
- `.wheel_scroll_multiplier(mut self, multiplier: Vec2) -> Self` — The scroll amount caused by a mouse wheel scroll is multiplied by this amount.

**ScrollAnimation**
- `.duration(t: f32) -> Self` — Scroll with a fixed duration, regardless of distance.
- `.new(points_per_second: f32, duration: Rangef) -> Self` — New scroll animation
- `.none() -> Self` — No animation, scroll instantly.
- `.ui(&mut self, ui: &mut crate::Ui)`

**ScrollTarget**
- `.new(range: Rangef, align: Option<Align>, animation: style::ScrollAnimation) -> Self`

**ScrollSource**
- `.any(&self) -> bool` — Is anything enabled?
- `.is_all(&self) -> bool` — Is everything enabled?
- `.is_none(&self) -> bool` — Is everything disabled?

**Grid**
- `.max_col_width(mut self, max_col_width: f32) -> Self` — Set soft maximum width (wrapping width) of each column.
- `.min_col_width(mut self, min_col_width: f32) -> Self` — Set minimum width of each column.
- `.min_row_height(mut self, min_row_height: f32) -> Self` — Set minimum height of each row.
- `.new(id_salt: impl AsIdSalt) -> Self` — Create a new Grid with a locally unique identifier.
- `.num_columns(mut self, num_columns: usize) -> Self` — Setting this will allow the last column to expand to take up the rest of the space of the parent Ui.
- `.show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>`
- `.spacing(mut self, spacing: impl Into<Vec2>) -> Self` — Set spacing between columns/rows.
- `.start_row(mut self, start_row: usize) -> Self` — Change which row number the grid starts on.
- `.striped(self, striped: bool) -> Self` — If true, add a subtle background color to every other row.
- `.with_row_color<F>(mut self, color_picker: F) -> Self where F: Send + Sync + Fn(usize, &Style) -> Option<Color32> + 'static,` — Setting this will allow for dynamic coloring of rows of the grid object

**Region**
- `.expand_to_include_rect(&mut self, rect: Rect)` — Expand the min_rect and max_rect of this ui to include a child at the given rect.
- `.expand_to_include_x(&mut self, x: f32)` — Ensure we are big enough to contain the given X-coordinate.
- `.expand_to_include_y(&mut self, y: f32)` — Ensure we are big enough to contain the given Y-coordinate.
- `.sanity_check(&self)`

## Frame & Scene

**Frame**
- `.begin(self, ui: &mut Ui) -> Prepared` — Begin a dynamically colored frame.
- `.canvas(style: &Style) -> Self` — A canvas to draw on.
- `.central_panel(style: &Style) -> Self`
- `.corner_radius(mut self, corner_radius: impl Into<CornerRadius>) -> Self` — The rounding of the _outer_ corner of the Self::stroke (or, if there is no stroke, the outer corner of Self::fill).
- `.dark_canvas(style: &Style) -> Self` — A dark canvas to draw on.
- `.fill(mut self, fill: Color32) -> Self` — The background fill color of the frame, within the Self::stroke.
- `.fill_rect(&self, content_rect: Rect) -> Rect` — Calculate the fill_rect from the content_rect.
- `.group(style: &Style) -> Self` — For when you want to group a few widgets together within a frame.
- `.inner_margin(mut self, inner_margin: impl Into<Margin>) -> Self` — Margin within the painted frame.
- `.menu(style: &Style) -> Self`
- `.multiply_with_opacity(mut self, opacity: f32) -> Self` — Opacity multiplier in gamma space.
- `.new() -> Self` — No colors, no margins, no border.
- `.outer_margin(mut self, outer_margin: impl Into<Margin>) -> Self` — Margin outside the painted frame.
- `.outer_rect(&self, content_rect: Rect) -> Rect` — Calculate the outer_rect from the content_rect.
- `.paint(&self, content_rect: Rect) -> Shape` — Paint this frame as a shape.
- `.popup(style: &Style) -> Self`
- `.shadow(mut self, shadow: Shadow) -> Self` — Optional drop-shadow behind the frame.
- `.show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>` — Show the given ui surrounded by this frame.
- `.show_dyn<'c, R>(self, ui: &mut Ui, add_contents: Box<dyn FnOnce(&mut Ui) -> R + 'c>,) -> InnerResponse<R>` — Show using dynamic dispatch.
- `.side_top_panel(style: &Style) -> Self`
- `.stroke(mut self, stroke: impl Into<Stroke>) -> Self` — The width and color of the outline around the frame.
- `.total_margin(&self) -> MarginF32` — How much extra space the frame uses up compared to the content.
- `.widget_rect(&self, content_rect: Rect) -> Rect` — Calculate the widget_rect from the content_rect.
- `.window(style: &Style) -> Self` — The default frame for an crate::Window.

**Scene**
- `.drag_pan_buttons(mut self, flags: DragPanButtons) -> Self` — Specify which pointer buttons can be used to pan by clicking and dragging.
- `.max_inner_size(mut self, max_inner_size: impl Into<Vec2>) -> Self` — Set the maximum size of the inner Ui that will be created.
- `.new() -> Self`
- `.register_pan_and_zoom(&self, ui: &Ui, resp: &mut Response, to_global: &mut TSTransform)` — Helper function to handle pan and zoom interactions on a response.
- `.sense(mut self, sense: Sense) -> Self` — Specify what type of input the scene should respond to.
- `.show<R>(&self, parent_ui: &mut Ui, scene_rect: &mut Rect, add_contents: impl FnOnce(&mut Ui) -> R,) -> InnerResponse<R>` — scene_rect contains the view bounds of the inner Ui.
- `.zoom_range(mut self, zoom_range: impl Into<Rangef>) -> Self` — Set the allowed zoom range.

## Popup / Tooltip / Modal / Menu

**Popup**
- `.align(mut self, position_align: RectAlign) -> Self` — Set the RectAlign of the popup relative to the PopupAnchor.
- `.align_alternatives(mut self, alternatives: &'a [RectAlign]) -> Self` — Set alternative positions to try if the default one doesn't fit.
- `.anchor(mut self, anchor: impl Into<PopupAnchor>) -> Self` — Show the popup relative to the given PopupAnchor.
- `.at_pointer(mut self) -> Self` — Show the popup relative to the pointer.
- `.at_pointer_fixed(mut self) -> Self` — Remember the pointer position at the time of opening the popup, and show the popup relative to that.
- `.at_position(mut self, position: Pos2) -> Self` — Show the popup relative to a specific position.
- `.close_all(ctx: &Context)` — Close all currently open popups.
- `.close_behavior(mut self, close_behavior: PopupCloseBehavior) -> Self` — Set the close behavior of the popup.
- `.close_id(ctx: &Context, popup_id: Id)` — Close the given popup, if it is open.
- `.context_menu(response: &Response) -> Self` — Show a context menu when the widget was secondary clicked.
- `.ctx(&self) -> &Context` — Get the Context
- `.default_response_id(response: &Response) -> Id` — The default ID when constructing a popup from the Response of e.g. a button.
- `.frame(mut self, frame: Frame) -> Self` — Set the frame of the popup.
- `.from_response(response: &Response) -> Self` — Show a popup relative to some widget.
- `.from_toggle_button_response(button_response: &Response) -> Self` — Show a popup relative to some widget, toggling the open state based on the widget's click state.
- `.gap(mut self, gap: f32) -> Self` — Set the gap between the anchor and the popup.
- `.get_anchor(&self) -> PopupAnchor` — Return the PopupAnchor of the popup.
- `.get_anchor_rect(&self) -> Option<Rect>` — Return the anchor rect of the popup.
- `.get_best_align(&self) -> RectAlign` — Calculate the best alignment for the popup, based on the last size and screen rect.
- `.get_expected_size(&self) -> Option<Vec2>` — Get the expected size of the popup.
- `.get_id(&self) -> Id` — Get the id of the popup.
- `.get_popup_rect(&self) -> Option<Rect>` — Get the expected rect the popup will be shown in.
- `.id(mut self, id: Id) -> Self` — Set the id of the Area.
- `.info(mut self, info: UiStackInfo) -> Self` — Set the UiStackInfo of the popup's Ui.
- `.is_any_open(ctx: &Context) -> bool` — Is any popup open?
- `.is_id_open(ctx: &Context, popup_id: Id) -> bool` — Is the given popup open?
- `.is_open(&self) -> bool` — Is the popup open?
- `.kind(mut self, kind: PopupKind) -> Self` — Set the kind of the popup.
- `.layout(mut self, layout: Layout) -> Self` — Set the layout of the popup.
- `.menu(button_response: &Response) -> Self` — Show a popup when the widget was clicked.
- `.new(id: Id, ctx: Context, anchor: impl Into<PopupAnchor>, layer_id: LayerId) -> Self` — Create a new popup
- `.open(mut self, open: bool) -> Self` — Force the popup to be open or closed.
- `.open_bool(mut self, open: &'a mut bool) -> Self` — Store the open state via a mutable bool.
- `.open_id(ctx: &Context, popup_id: Id)` — Open the given popup and close all others.
- `.open_memory(mut self, set_state: impl Into<Option<SetOpenCommand>>) -> Self` — Store the open state via crate::Memory.
- `.position_of_id(ctx: &Context, popup_id: Id) -> Option<Pos2>` — Get the position for this popup, if it is open.
- `.sense(mut self, sense: Sense) -> Self` — Set the sense of the popup.
- `.show<R>(self, content: impl FnOnce(&mut Ui) -> R) -> Option<InnerResponse<R>>` — Show the popup.
- `.style(mut self, style: impl Into<StyleModifier>) -> Self` — Set the style for the popup contents.
- `.toggle_id(ctx: &Context, popup_id: Id)` — Toggle the given popup between closed and open.
- `.width(mut self, width: f32) -> Self` — The width that will be passed to Area::default_width.

**PopupAnchor**
- `.rect(self, popup_id: Id, ctx: &Context) -> Option<Rect>` — Get the rect the popup should be shown relative to.

**PopupKind**
- `.order(self) -> Order` — Returns the order to be used with this kind.

**Tooltip**
- `.always_open(ctx: Context, parent_layer: LayerId, parent_widget: Id, anchor: impl Into<PopupAnchor>,) -> Self` — Show a tooltip that is always open.
- `.at_pointer(mut self) -> Self` — Show the tooltip at the pointer position.
- `.for_disabled(response: &Response) -> Self` — Show a tooltip when hovering a disabled widget.
- `.for_enabled(response: &Response) -> Self` — Show a tooltip when hovering an enabled widget.
- `.for_widget(response: &Response) -> Self` — Show a tooltip for a widget.
- `.gap(mut self, gap: f32) -> Self` — Set the gap between the tooltip and the anchor
- `.layout(mut self, layout: Layout) -> Self` — Set the layout of the tooltip
- `.next_tooltip_id(ctx: &Context, widget_id: Id) -> Id` — What is the id of the next tooltip for this widget?
- `.seconds_since_last_tooltip(ctx: &Context) -> f32`
- `.should_show_tooltip(response: &Response, allow_interactive_tooltip: bool) -> bool` — Should we show a tooltip for this response?
- `.show<R>(self, content: impl FnOnce(&mut crate::Ui) -> R) -> Option<InnerResponse<R>>` — Show the tooltip
- `.tooltip_id(widget_id: Id, tooltip_count: usize) -> Id`
- `.was_tooltip_open_last_frame(ctx: &Context, widget_id: Id) -> bool` — Was this tooltip visible last frame?
- `.width(mut self, width: f32) -> Self` — Set the width of the tooltip

**TooltipPassState**
- `.clear(&mut self)`

**Modal**
- `.area(mut self, area: Area) -> Self` — Set the area of the modal.
- `.backdrop_color(mut self, color: Color32) -> Self` — Set the backdrop color of the modal.
- `.default_area(id: Id) -> Area` — Returns an area customized for a modal.
- `.frame(mut self, frame: Frame) -> Self` — Set the frame of the modal.
- `.new(id: Id) -> Self` — Create a new Modal.
- `.show<T>(self, ctx: &Context, content: impl FnOnce(&mut Ui) -> T) -> ModalResponse<T>` — Show the modal.

**MenuBar**
- `.config(mut self, config: MenuConfig) -> Self` — Set the config for submenus.
- `.new() -> Self`
- `.style(mut self, style: impl Into<StyleModifier>) -> Self` — Set the style for buttons in the menu bar.
- `.ui<R>(self, ui: &mut Ui, content: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>` — Show the menu bar.

**MenuButton**
- `.config(mut self, config: MenuConfig) -> Self` — Set the config for the menu.
- `.from_button(button: Button<'a>) -> Self` — Create a new menu button from a Button.
- `.new(atoms: impl IntoAtoms<'a>) -> Self`
- `.ui<R>(self, ui: &mut Ui, content: impl FnOnce(&mut Ui) -> R,) -> (Response, Option<InnerResponse<R>>)` — Show the menu button.

**MenuConfig**
- `.close_behavior(mut self, close_behavior: PopupCloseBehavior) -> Self` — If the user clicks, should we close the menu?
- `.find(ui: &Ui) -> Self` — Find the config for the current menu.
- `.new() -> Self`
- `.style(mut self, style: impl Into<StyleModifier>) -> Self` — Override the menu style.

**MenuState**
- `.from_id<R>(ctx: &Context, id: Id, f: impl FnOnce(&mut Self) -> R) -> R` — Get the state via the menus root Ui id
- `.from_ui<R>(ui: &Ui, f: impl FnOnce(&mut Self, &UiStack) -> R) -> R` — Find the root of the menu and get the state
- `.is_deepest_open_sub_menu(ctx: &Context, id: Id) -> bool` — Is the menu with this id the deepest sub menu? (-> no child sub menu is open)
- `.mark_shown(ctx: &Context, id: Id)`

**SubMenu**
- `.config(mut self, config: MenuConfig) -> Self` — Set the config for the submenu.
- `.id_from_widget_id(widget_id: Id) -> Id` — Get the id for the submenu from the widget/response id.
- `.new() -> Self`
- `.show<R>(self, ui: &Ui, button_response: &Response, content: impl FnOnce(&mut Ui) -> R,) -> Option<InnerResponse<R>>` — Show the submenu.

**SubMenuButton**
- `.config(mut self, config: MenuConfig) -> Self` — Set the config for the submenu.
- `.from_button(button: Button<'a>) -> Self` — Create a new submenu button from a Button.
- `.new(atoms: impl IntoAtoms<'a>) -> Self`
- `.ui<R>(self, ui: &mut Ui, content: impl FnOnce(&mut Ui) -> R,) -> (Response, Option<InnerResponse<R>>)` — Show the submenu button.

## CollapsingHeader

**CollapsingHeader**
- `.default_open(mut self, open: bool) -> Self` — By default, the CollapsingHeader is collapsed.
- `.enabled(mut self, enabled: bool) -> Self` — If you set this to false, the CollapsingHeader will be grayed out and un-clickable.
- `.icon(mut self, icon_fn: impl FnOnce(&mut Ui, f32, &Response) + 'static) -> Self` — Use the provided function to render a different CollapsingHeader icon.
- `.id_salt(mut self, id_salt: impl AsIdSalt) -> Self` — Explicitly set the source of the Id of this widget, instead of using title label.
- `.new(text: impl Into<WidgetText>) -> Self` — The CollapsingHeader starts out collapsed unless you call default_open.
- `.open(mut self, open: Option<bool>) -> Self` — Calling .open(Some(true)) will make the collapsing header open this frame (or stay open).
- `.show<R>(self, ui: &mut Ui, add_body: impl FnOnce(&mut Ui) -> R,) -> CollapsingResponse<R>`
- `.show_background(mut self, show_background: bool) -> Self` — Should the CollapsingHeader show a background behind it? Default: false.
- `.show_unindented<R>(self, ui: &mut Ui, add_body: impl FnOnce(&mut Ui) -> R,) -> CollapsingResponse<R>`

**CollapsingState**
- `.id(&self) -> Id`
- `.is_open(&self) -> bool`
- `.load(ctx: &Context, id: Id) -> Option<Self>`
- `.load_with_default_open(ctx: &Context, id: Id, default_open: bool) -> Self`
- `.openness(&self, ctx: &Context) -> f32` — 0 for closed, 1 for open, with tweening
- `.remove(&self, ctx: &Context)`
- `.set_open(&mut self, open: bool)`
- `.show_body_indented<R>(&mut self, header_response: &Response, ui: &mut Ui, add_body: impl FnOnce(&mut Ui) -> R,) -> Option<InnerResponse<R>>` — Show body if we are open, with a nice animation between closed and open.
- `.show_body_unindented<R>(&mut self, ui: &mut Ui, add_body: impl FnOnce(&mut Ui) -> R,) -> Option<InnerResponse<R>>` — Show body if we are open, with a nice animation between closed and open.
- `.show_header<HeaderRet>(mut self, ui: &mut Ui, add_header: impl FnOnce(&mut Ui) -> HeaderRet,) -> HeaderResponse<'_, HeaderRet>` — Shows header and body (if expanded).
- `.show_toggle_button(&mut self, ui: &mut Ui, icon_fn: impl FnOnce(&mut Ui, f32, &Response) + 'static,) -> Response` — Paint this CollapsingState's toggle button.
- `.store(&self, ctx: &Context)`
- `.toggle(&mut self, ui: &Ui)`

## Widgets — Button / Label / etc.

**Button**
- `.atom_ui(self, ui: &mut Ui) -> AtomLayoutResponse` — Show the button and return a AtomLayoutResponse for painting custom contents.
- `.corner_radius(mut self, corner_radius: impl Into<CornerRadius>) -> Self` — Set the rounding of the button.
- `.fill(mut self, fill: impl Into<Color32>) -> Self` — Override background fill color.
- `.frame(mut self, frame: bool) -> Self` — Turn off the frame
- `.frame_when_inactive(mut self, frame_when_inactive: bool) -> Self` — If false, the button will not have a frame when inactive.
- `.gap(mut self, gap: f32) -> Self` — Set the gap between atoms.
- `.image(image: impl Into<Image<'a>>) -> Self` — Creates a button with an image.
- `.image_and_text(image: impl Into<Image<'a>>, text: impl Into<WidgetText>) -> Self` — Creates a button with an image to the left of the text.
- `.image_tint_follows_text_color(mut self, image_tint_follows_text_color: bool) -> Self` — If true, the tint of the image is multiplied by the widget text color.
- `.left_text(mut self, left_text: impl IntoAtoms<'a>) -> Self` — Show some text on the left side of the button.
- `.min_size(mut self, min_size: Vec2) -> Self` — Set the minimum size of the button.
- `.new(atoms: impl IntoAtoms<'a>) -> Self`
- `.opt_image_and_text(image: Option<Image<'a>>, text: Option<WidgetText>) -> Self` — Create a button with an optional image and optional text.
- `.right_text(mut self, right_text: impl IntoAtoms<'a>) -> Self` — Show some text on the right side of the button.
- `.selectable(selected: bool, atoms: impl IntoAtoms<'a>) -> Self` — Show a selectable button.
- `.selected(mut self, selected: bool) -> Self` — If true, mark this button as "selected".
- `.sense(mut self, sense: Sense) -> Self` — By default, buttons senses clicks.
- `.shortcut_text(mut self, shortcut_text: impl IntoAtoms<'a>) -> Self` — Show some text on the right side of the button, in weak color.
- `.small(mut self) -> Self` — Make this a small button, suitable for embedding into text.
- `.stroke(mut self, stroke: impl Into<Stroke>) -> Self` — Override button stroke.
- `.truncate(self) -> Self` — Set Self::wrap_mode to TextWrapMode::Truncate.
- `.wrap(self) -> Self` — Set Self::wrap_mode to TextWrapMode::Wrap.
- `.wrap_mode(mut self, wrap_mode: TextWrapMode) -> Self` — Set the wrap mode for the text.

**Label**
- `.extend(mut self) -> Self` — Set Self::wrap_mode to TextWrapMode::Extend, disabling wrapping and truncating, and instead expanding the parent Ui.
- `.halign(mut self, align: Align) -> Self` — Sets the horizontal alignment of the Label to the given Align value.
- `.layout_in_ui(self, ui: &mut Ui) -> (Pos2, Arc<Galley>, Response)` — Do layout and position the galley in the ui, without painting it or adding widget info.
- `.new(text: impl Into<WidgetText>) -> Self`
- `.selectable(mut self, selectable: bool) -> Self` — Can the user select the text with the mouse?
- `.sense(mut self, sense: Sense) -> Self` — Make the label respond to clicks and/or drags.
- `.show_tooltip_when_elided(mut self, show: bool) -> Self` — Show the full text when hovered, if the text was elided.
- `.text(&self) -> &str`
- `.truncate(mut self) -> Self` — Set Self::wrap_mode to TextWrapMode::Truncate.
- `.wrap(mut self) -> Self` — Set Self::wrap_mode to TextWrapMode::Wrap.
- `.wrap_mode(mut self, wrap_mode: TextWrapMode) -> Self` — Set the wrap mode for the text.

**Checkbox**
- `.indeterminate(mut self, indeterminate: bool) -> Self` — Display an indeterminate state (neither checked nor unchecked)
- `.new(checked: &'a mut bool, atoms: impl IntoAtoms<'a>) -> Self`
- `.without_text(checked: &'a mut bool) -> Self`

**RadioButton**
- `.new(checked: bool, atoms: impl IntoAtoms<'a>) -> Self`

**Hyperlink**
- `.from_label_and_url(text: impl Into<WidgetText>, url: impl ToString) -> Self`
- `.new(url: impl ToString) -> Self`
- `.open_in_new_tab(mut self, new_tab: bool) -> Self` — Always open this hyperlink in a new browser tab.

**Link**
- `.new(text: impl Into<WidgetText>) -> Self`

**Separator**
- `.grow(mut self, extra: f32) -> Self` — Extend each end of the separator line by this much.
- `.horizontal(mut self) -> Self` — Explicitly ask for a horizontal line.
- `.shrink(mut self, shrink: f32) -> Self` — Contract each end of the separator line by this much.
- `.spacing(mut self, spacing: f32) -> Self` — How much space we take up.
- `.vertical(mut self) -> Self` — Explicitly ask for a vertical line.

**Spinner**
- `.color(mut self, color: impl Into<Color32>) -> Self` — Sets the spinner's color.
- `.new() -> Self` — Create a new spinner that uses the style's interact_size unless changed.
- `.paint_at(&self, ui: &Ui, rect: Rect)` — Paint the spinner in the given rectangle.
- `.size(mut self, size: f32) -> Self` — Sets the spinner's size.

**ProgressBar**
- `.animate(mut self, animate: bool) -> Self` — Whether to display a loading animation when progress < 1.
- `.corner_radius(mut self, corner_radius: impl Into<CornerRadius>) -> Self` — Set the rounding of the progress bar.
- `.desired_height(mut self, desired_height: f32) -> Self` — The desired height of the bar.
- `.desired_width(mut self, desired_width: f32) -> Self` — The desired width of the bar.
- `.fill(mut self, color: Color32) -> Self` — The fill color of the bar.
- `.new(progress: f32) -> Self` — Progress in the [0, 1] range, where 1 means "completed".
- `.show_percentage(mut self) -> Self` — Show the progress in percent on the progress bar.
- `.text(mut self, text: impl Into<WidgetText>) -> Self` — A custom text to display on the progress bar.

**Slider**
- `.binary(self, min_width: usize, twos_complement: bool) -> Self` — Set custom_formatter and custom_parser to display and parse numbers as binary integers.
- `.clamping(mut self, clamping: SliderClamping) -> Self` — Controls when the values will be clamped to the range.
- `.custom_formatter(mut self, formatter: impl 'a + Fn(f64, RangeInclusive<usize>) -> String,) -> Self` — Set custom formatter defining how numbers are converted into text.
- `.custom_parser(mut self, parser: impl 'a + Fn(&str) -> Option<f64>) -> Self` — Set custom parser defining how the text input is parsed into a number.
- `.drag_value_speed(mut self, drag_value_speed: f64) -> Self` — When dragging the value, how fast does it move?
- `.fixed_decimals(mut self, num_decimals: usize) -> Self` — Set an exact number of decimals to display.
- `.from_get_set(range: RangeInclusive<f64>, get_set_value: impl 'a + FnMut(Option<f64>) -> f64,) -> Self`
- `.handle_shape(mut self, handle_shape: HandleShape) -> Self` — Change the shape of the slider handle
- `.hexadecimal(self, min_width: usize, twos_complement: bool, upper: bool) -> Self` — Set custom_formatter and custom_parser to display and parse numbers as hexadecimal integers.
- `.integer(self) -> Self` — Helper: equivalent to self.precision(0).smallest_positive(1.0).
- `.largest_finite(mut self, largest_finite: f64) -> Self` — For logarithmic sliders, the largest positive value we are interested in before the slider switches to INFINITY, if that is the higher end.
- `.logarithmic(mut self, logarithmic: bool) -> Self` — Make this a logarithmic slider.
- `.max_decimals(mut self, max_decimals: usize) -> Self` — Set a maximum number of decimals to display.
- `.max_decimals_opt(mut self, max_decimals: Option<usize>) -> Self`
- `.min_decimals(mut self, min_decimals: usize) -> Self` — Set a minimum number of decimals to display.
- `.new<Num: emath::Numeric>(value: &'a mut Num, range: RangeInclusive<Num>) -> Self` — Creates a new horizontal slider.
- `.octal(self, min_width: usize, twos_complement: bool) -> Self` — Set custom_formatter and custom_parser to display and parse numbers as octal integers.
- `.orientation(mut self, orientation: SliderOrientation) -> Self` — Vertical or horizontal slider? The default is horizontal.
- `.prefix(mut self, prefix: impl ToString) -> Self` — Show a prefix before the number, e.g. "x: "
- `.show_value(mut self, show_value: bool) -> Self` — Control whether or not the slider shows the current value.
- `.smallest_positive(mut self, smallest_positive: f64) -> Self` — For logarithmic sliders that includes zero: what is the smallest positive value you want to be able to select? The default is 1 for integer sliders and 1e-6...
- `.smart_aim(mut self, smart_aim: bool) -> Self` — Turn smart aim on/off.
- `.step_by(mut self, step: f64) -> Self` — Sets the minimal change of the value.
- `.suffix(mut self, suffix: impl ToString) -> Self` — Add a suffix to the number, this can be e.g. a unit ("°" or " m")
- `.text(mut self, text: impl Into<WidgetText>) -> Self` — Show a text next to the slider (e.g. explaining what the slider controls).
- `.text_color(mut self, text_color: Color32) -> Self`
- `.trailing_fill(mut self, trailing_fill: bool) -> Self` — Display trailing color behind the slider's circle.
- `.update_while_editing(mut self, update: bool) -> Self` — Update the value on each key press when text-editing the value.
- `.vertical(mut self) -> Self` — Make this a vertical slider.

**DragValue**
- `.binary(self, min_width: usize, twos_complement: bool) -> Self` — Set custom_formatter and custom_parser to display and parse numbers as binary integers.
- `.clamp_existing_to_range(mut self, clamp_existing_to_range: bool) -> Self` — If set to true, existing values will be clamped to Self::range.
- `.custom_formatter(mut self, formatter: impl 'a + Fn(f64, RangeInclusive<usize>) -> String,) -> Self` — Set custom formatter defining how numbers are converted into text.
- `.custom_parser(mut self, parser: impl 'a + Fn(&str) -> Option<f64>) -> Self` — Set custom parser defining how the text input is parsed into a number.
- `.fixed_decimals(mut self, num_decimals: usize) -> Self` — Set an exact number of decimals to display.
- `.from_get_set(get_set_value: impl 'a + FnMut(Option<f64>) -> f64) -> Self`
- `.hexadecimal(self, min_width: usize, twos_complement: bool, upper: bool) -> Self` — Set custom_formatter and custom_parser to display and parse numbers as hexadecimal integers.
- `.max_decimals(mut self, max_decimals: usize) -> Self` — Set a maximum number of decimals to display.
- `.max_decimals_opt(mut self, max_decimals: Option<usize>) -> Self`
- `.min_decimals(mut self, min_decimals: usize) -> Self` — Set a minimum number of decimals to display.
- `.new<Num: emath::Numeric>(value: &'a mut Num) -> Self`
- `.octal(self, min_width: usize, twos_complement: bool) -> Self` — Set custom_formatter and custom_parser to display and parse numbers as octal integers.
- `.prefix(mut self, prefix: impl IntoAtoms<'a>) -> Self` — Show a prefix before the number, e.g. "x: "
- `.range<Num: emath::Numeric>(mut self, range: RangeInclusive<Num>) -> Self` — Sets valid range for dragging the value.
- `.speed(mut self, speed: impl Into<f64>) -> Self` — How much the value changes when dragged one point (logical pixel).
- `.suffix(mut self, suffix: impl IntoAtoms<'a>) -> Self` — Add a suffix to the number, this can be e.g. a unit ("°" or " m")
- `.update_while_editing(mut self, update: bool) -> Self` — Update the value on each key press when text-editing the value.

**ComboBox**
- `.close_behavior(mut self, close_behavior: PopupCloseBehavior) -> Self` — Controls the close behavior for the popup.
- `.from_id_salt(id_salt: impl AsIdSalt) -> Self` — Without label.
- `.from_label(label: impl Into<WidgetText>) -> Self` — Label shown next to the combo box
- `.height(mut self, height: f32) -> Self` — Set the maximum outer height of the menu.
- `.icon(mut self, icon_fn: impl FnOnce(&Ui, Rect, &WidgetVisuals, bool) + 'static) -> Self` — Use the provided function to render a different ComboBox icon.
- `.is_open(ctx: &Context, id: Id) -> bool` — Check if the ComboBox with the given id has its popup menu currently opened.
- `.new(id_salt: impl AsIdSalt, label: impl Into<WidgetText>) -> Self` — Create new ComboBox with id and label
- `.popup_style(mut self, popup_style: StyleModifier) -> Self` — Set the style of the popup menu.
- `.selected_text(mut self, selected_text: impl Into<WidgetText>) -> Self` — What we show as the currently selected value
- `.show_index<Text: Into<WidgetText>>(self, ui: &mut Ui, selected: &mut usize, len: usize, get: impl Fn(usize) -> Text,) -> Response` — Show a list of items with the given selected index.
- `.show_ui<R>(self, ui: &mut Ui, menu_contents: impl FnOnce(&mut Ui) -> R,) -> InnerResponse<Option<R>>` — Show the combo box, with the given ui code for the menu contents.
- `.truncate(mut self) -> Self` — Set Self::wrap_mode to TextWrapMode::Truncate.
- `.width(mut self, width: f32) -> Self` — Set the outer width of the button and menu.
- `.wrap(mut self) -> Self` — Set Self::wrap_mode to TextWrapMode::Wrap.
- `.wrap_mode(mut self, wrap_mode: TextWrapMode) -> Self` — Controls the wrap mode used for the selected text.

**Image**
- `.alt_text(mut self, label: impl Into<String>) -> Self` — Set alt text for the image.
- `.bg_fill(mut self, bg_fill: impl Into<Color32>) -> Self` — A solid color to put behind the image.
- `.calc_size(&self, available_size: Vec2, image_source_size: Option<Vec2>) -> Vec2` — Returns the size the image will occupy in the final UI.
- `.corner_radius(mut self, corner_radius: impl Into<CornerRadius>) -> Self` — Round the corners of the image.
- `.fit_to_exact_size(mut self, size: Vec2) -> Self` — Fit the image to an exact size.
- `.fit_to_fraction(mut self, fraction: Vec2) -> Self` — Fit the image to a fraction of the available space.
- `.fit_to_original_size(mut self, scale: f32) -> Self` — Fit the image to its original size with some scaling.
- `.from_bytes(uri: impl Into<Cow<'static, str>>, bytes: impl Into<Bytes>) -> Self` — Load the image from some raw bytes.
- `.from_texture(texture: impl Into<SizedTexture>) -> Self` — Load the image from an existing texture.
- `.from_uri(uri: impl Into<Cow<'a, str>>) -> Self` — Load the image from a URI.
- `.image_options(&self) -> &ImageOptions`
- `.load_and_calc_size(&self, ui: &Ui, available_size: Vec2) -> Option<Vec2>`
- `.load_for_size(&self, ctx: &Context, available_size: Vec2) -> TextureLoadResult` — Load the image from its Image::source, returning the resulting SizedTexture.
- `.maintain_aspect_ratio(mut self, value: bool) -> Self` — Whether or not the ImageFit should maintain the image's original aspect ratio.
- `.max_height(mut self, height: f32) -> Self` — Set the max height of the image.
- `.max_size(mut self, size: Vec2) -> Self` — Set the max size of the image.
- `.max_width(mut self, width: f32) -> Self` — Set the max width of the image.
- `.new(source: impl Into<ImageSource<'a>>) -> Self` — Load the image from some source.
- `.paint_at(&self, ui: &Ui, rect: Rect)` — Paint the image in the given rectangle.
- `.rotate(mut self, angle: f32, origin: Vec2) -> Self` — Rotate the image about an origin by some angle
- `.sense(mut self, sense: Sense) -> Self` — Make the image respond to clicks and/or drags.
- `.show_loading_spinner(mut self, show: bool) -> Self` — Show a spinner when the image is loading.
- `.shrink_to_fit(self) -> Self` — Fit the image to 100% of its available size, shrinking it if necessary.
- `.size(&self) -> Option<Vec2>`
- `.source(&'a self, ctx: &Context) -> ImageSource<'a>`
- `.texture_options(mut self, texture_options: TextureOptions) -> Self` — Texture options used when creating the texture.
- `.tint(mut self, tint: impl Into<Color32>) -> Self` — Multiply image color with this.
- `.uri(&self) -> Option<&str>` — Returns the URI of the image.
- `.uv(mut self, uv: impl Into<Rect>) -> Self` — Select UV range.

**ImageSource**
- `.load(self, ctx: &Context, texture_options: TextureOptions, size_hint: SizeHint,) -> TextureLoadResult` — Failure to load the texture.
- `.texture_size(&self) -> Option<Vec2>` — Size of the texture, if known.
- `.uri(&self) -> Option<&str>` — Get the uri that this image was constructed from.

**ImageData**
- `.bytes_per_pixel(&self) -> usize`
- `.height(&self) -> usize`
- `.size(&self) -> [usize; 2]`
- `.width(&self) -> usize`

**ImageDelta**
- `.full(image: impl Into<ImageData>, options: TextureOptions) -> Self` — Update the whole texture.
- `.is_whole(&self) -> bool` — Is this affecting the whole texture? If false, this is a partial (sub-region) update.
- `.partial(pos: [usize; 2], image: impl Into<ImageData>, options: TextureOptions) -> Self` — Update a sub-region of an existing texture.

**ImageFit**
- `.resolve(self, available_size: Vec2, image_size: Vec2) -> Vec2`

**ImageSize**
- `.calc_size(&self, available_size: Vec2, image_source_size: Vec2) -> Vec2` — Calculate the final on-screen size in points.
- `.hint(&self, available_size: Vec2, pixels_per_point: f32) -> SizeHint` — Size hint for e.g. rasterizing an svg.

**NumberFormatter**
- `.format(&self, value: f64, decimals: RangeInclusive<usize>) -> String` — Format the given number with the given number of decimals.
- `.new(formatter: impl 'static + Sync + Send + Fn(f64, RangeInclusive<usize>) -> String,) -> Self` — The first argument is the number to be formatted.

**HandleShape**
- `.ui(&mut self, ui: &mut Ui)`

## TextEdit

**TextEdit**
- `.background_color(mut self, color: Color32) -> Self` — Set the background color of the TextEdit.
- `.char_limit(mut self, limit: usize) -> Self` — Sets the limit for the amount of characters can be entered
- `.clip_text(mut self, b: bool) -> Self` — When true (default), overflowing text will be clipped.
- `.code_editor(self) -> Self` — Build a TextEdit focused on code editing.
- `.cursor_at_end(mut self, b: bool) -> Self` — When true (default), the cursor will initially be placed at the end of the text.
- `.desired_rows(mut self, desired_height_rows: usize) -> Self` — Set the number of rows to show by default.
- `.desired_width(mut self, desired_width: f32) -> Self` — Set to 0.0 to keep as small as possible.
- `.font(mut self, font_selection: impl Into<FontSelection>) -> Self` — Pick a crate::FontId or TextStyle.
- `.frame(mut self, frame: Frame) -> Self` — Customize the Frame around the text edit.
- `.hint_text(mut self, hint_text: impl IntoAtoms<'static>) -> Self` — Show a faint hint text when the text field is empty.
- `.horizontal_align(mut self, align: Align) -> Self` — Set the horizontal align of the inner text.
- `.id(mut self, id: Id) -> Self` — Use if you want to set an explicit Id for this widget.
- `.id_salt(mut self, id_salt: impl AsIdSalt) -> Self` — A source for the unique Id, e.g. .id_salt("second_text_edit_field") or .id_salt(loop_index).
- `.id_source(self, id_salt: impl AsIdSalt) -> Self` — A source for the unique Id, e.g. .id_source("second_text_edit_field") or .id_source(loop_index).
- `.interactive(mut self, interactive: bool) -> Self` — Default is true.
- `.layouter(mut self, layouter: &'t mut dyn FnMut(&Ui, &dyn TextBuffer, f32) -> Arc<Galley>,) -> Self` — Override how text is being shown inside the TextEdit.
- `.load_state(ctx: &Context, id: Id) -> Option<TextEditState>`
- `.lock_focus(mut self, tab_will_indent: bool) -> Self` — When false (default), pressing TAB will move focus to the next widget.
- `.margin(mut self, margin: impl Into<Margin>) -> Self` — Set margin of text.
- `.min_size(mut self, min_size: Vec2) -> Self` — Set the minimum size of the TextEdit.
- `.multiline(text: &'t mut dyn TextBuffer) -> Self` — A TextEdit for multiple lines.
- `.password(mut self, password: bool) -> Self` — If true, hide the letters from view and prevent copying from the field.
- `.prefix(mut self, prefix: impl IntoAtoms<'static>) -> Self` — Add a prefix to the text edit.
- `.return_key(mut self, return_key: impl Into<Option<KeyboardShortcut>>) -> Self` — Set the return key combination.
- `.show(self, ui: &mut Ui) -> TextEditOutput` — Show the TextEdit, returning a rich TextEditOutput.
- `.singleline(text: &'t mut dyn TextBuffer) -> Self` — No newlines (\n) allowed.
- `.store_state(ctx: &Context, id: Id, state: TextEditState)`
- `.suffix(mut self, suffix: impl IntoAtoms<'static>) -> Self` — Add a suffix to the text edit.
- `.text_color(mut self, text_color: Color32) -> Self`
- `.text_color_opt(mut self, text_color: Option<Color32>) -> Self`
- `.vertical_align(mut self, align: Align) -> Self` — Set the vertical align of the inner text.

**TextEditState**
- `.clear_undoer(&mut self)`
- `.load(ctx: &Context, id: Id) -> Option<Self>`
- `.set_undoer(&mut self, undoer: TextEditUndoer)`
- `.store(self, ctx: &Context, id: Id)`
- `.undoer(&self) -> TextEditUndoer`

**TextCursorState**
- `.char_range(&self) -> Option<CCursorRange>` — The currently selected range of characters.
- `.is_empty(&self) -> bool`
- `.pointer_interaction(&mut self, ui: &Ui, response: &Response, cursor_at_pointer: CCursor, galley: &Galley, is_being_dragged: bool,) -> bool` — Handle clicking and/or dragging text.
- `.range(&self, galley: &Galley) -> Option<CCursorRange>` — The currently selected range of characters, clamped within the character range of the given Galley.
- `.set_char_range(&mut self, ccursor_range: Option<CCursorRange>)` — Sets the currently selected range of characters.

**CCursor**
- `.new(index: impl Into<CharIndex>) -> Self`

**CCursorRange**
- `.as_sorted_char_range(&self) -> std::ops::Range<CharIndex>` — The range of selected character indices.
- `.contains(&self, other: Self) -> bool` — Is self a super-set of the other range?
- `.is_empty(&self) -> bool` — True if the selected range contains no characters.
- `.is_sorted(&self) -> bool`
- `.on_event(&mut self, os: OperatingSystem, event: &Event, galley: &Galley, _widget_id: Id,) -> bool` — Check for events that modify the cursor range.
- `.on_key_press(&mut self, os: OperatingSystem, galley: &Galley, modifiers: &Modifiers, key: Key,) -> bool` — Check for key presses that are moving the cursor.
- `.one(ccursor: CCursor) -> Self` — The empty range.
- `.select_all(galley: &Galley) -> Self` — Select all the text in a galley
- `.single(&self) -> Option<CCursor>` — If there is a selection, None is returned.
- `.slice_str<'s>(&self, text: &'s str) -> &'s str`
- `.sorted_cursors(&self) -> [CCursor; 2]` — returns the two ends ordered
- `.two(min: impl Into<CCursor>, max: impl Into<CCursor>) -> Self`

**LabelSelectionState**
- `.clear_selection(&mut self)` — Clear all label text selections in all viewports.
- `.has_selection(&self) -> bool` — Is there a label text selection in any viewport?
- `.label_text_selection(ui: &Ui, response: &Response, galley_pos: Pos2, mut galley: Arc<Galley>, fallback_color: epaint::Color32, underline: epaint::Stroke,)` — Handle text selection state for a label or similar widget.

**Selection**
- `.ui(&mut self, ui: &mut crate::Ui)`

## Text & Fonts

**RichText**
- `.append_to(self, layout_job: &mut LayoutJob, style: &Style, fallback_font: FontSelection, default_valign: Align,)` — Append to an existing LayoutJob
- `.background_color(mut self, background_color: impl Into<Color32>) -> Self` — Fill-color behind the text.
- `.code(mut self) -> Self` — Monospace label with different background color.
- `.color(mut self, color: impl Into<Color32>) -> Self` — Override text color.
- `.extra_letter_spacing(mut self, extra_letter_spacing: f32) -> Self` — Extra spacing between letters, in points.
- `.fallback_text_style(mut self, text_style: TextStyle) -> Self` — Set the TextStyle unless it has already been set
- `.family(mut self, family: FontFamily) -> Self` — Select the font family.
- `.font(mut self, font_id: crate::FontId) -> Self` — Select the font and size.
- `.font_height(&self, fonts: &mut epaint::FontsView<'_>, style: &Style) -> f32` — Read the font height of the selected text style.
- `.heading(self) -> Self` — Use TextStyle::Heading.
- `.is_empty(&self) -> bool`
- `.italics(mut self) -> Self` — Tilt the characters to the right.
- `.line_height(mut self, line_height: Option<f32>) -> Self` — Explicit line height of the text in points.
- `.monospace(self) -> Self` — Use TextStyle::Monospace.
- `.new(text: impl Into<String>) -> Self`
- `.raised(mut self) -> Self` — Align text to top.
- `.size(mut self, size: f32) -> Self` — Select the font size (in points).
- `.small(self) -> Self` — Smaller text.
- `.small_raised(self) -> Self` — For e.g. exponents.
- `.strikethrough(mut self) -> Self` — Draw a line through the text, crossing it out.
- `.strong(mut self) -> Self` — Extra strong text (stronger color).
- `.text(&self) -> &str`
- `.text_style(mut self, text_style: TextStyle) -> Self` — Override the TextStyle.
- `.underline(mut self) -> Self` — Draw a line under the text.
- `.variation(mut self, tag: impl IntoTag, coord: f32) -> Self` — Add a variation coordinate.
- `.variations<T: IntoTag>(mut self, variations: impl IntoIterator<Item = (T, f32)>,) -> Self` — Override the variation coordinates completely.
- `.weak(mut self) -> Self` — Extra weak text (fainter color).

**WidgetText**
- `.background_color(self, background_color: impl Into<Color32>) -> Self` — Prefer using RichText directly!
- `.code(self) -> Self` — Prefer using RichText directly!
- `.color(self, color: impl Into<Color32>) -> Self` — Override text color if, and only if, this is a RichText.
- `.fallback_text_style(self, text_style: TextStyle) -> Self` — Set the TextStyle unless it has already been set
- `.heading(self) -> Self` — Prefer using RichText directly!
- `.into_galley(self, ui: &Ui, wrap_mode: Option<TextWrapMode>, available_width: f32, fallback_font: impl Into<FontSelection>,) -> Arc<Galley>` — Layout with wrap mode based on the containing Ui.
- `.into_galley_impl(self, ctx: &crate::Context, style: &Style, text_wrapping: TextWrapping, fallback_font: FontSelection, default_valign: Align,) -> Arc<Galley>`
- `.into_layout_job(self, style: &Style, fallback_font: FontSelection, default_valign: Align,) -> Arc<LayoutJob>`
- `.is_empty(&self) -> bool`
- `.italics(self) -> Self` — Prefer using RichText directly!
- `.monospace(self) -> Self` — Prefer using RichText directly!
- `.raised(self) -> Self` — Prefer using RichText directly!
- `.small(self) -> Self` — Prefer using RichText directly!
- `.small_raised(self) -> Self` — Prefer using RichText directly!
- `.strikethrough(self) -> Self` — Prefer using RichText directly!
- `.strong(self) -> Self` — Prefer using RichText directly!
- `.text(&self) -> &str`
- `.text_style(self, text_style: TextStyle) -> Self` — Override the TextStyle if, and only if, this is a RichText.
- `.underline(self) -> Self` — Prefer using RichText directly!
- `.weak(self) -> Self` — Prefer using RichText directly!

**TextStyle**
- `.resolve(&self, style: &Style) -> FontId` — Look up this TextStyle in Style::text_styles.

**TextFormat**
- `.simple(font_id: FontId, color: Color32) -> Self`

**TextWrapping**
- `.from_wrap_mode_and_width(mode: TextWrapMode, max_width: f32) -> Self` — Create a TextWrapping from a TextWrapMode and an available width.
- `.no_max_width() -> Self` — A row can be as long as it need to be.
- `.truncate_at_width(max_width: f32) -> Self` — Elide text that doesn't fit within the given width, replaced with ….
- `.wrap_at_width(max_width: f32) -> Self` — A row can be at most max_width wide but can wrap in any number of lines.

**LayoutJob**
- `.append(&mut self, text: &str, leading_space: f32, format: TextFormat)` — Helper for adding a new section when building a LayoutJob.
- `.debug_sanity_check(&self)` — Check the Self::sections invariant: the sections are ordered and together cover the whole of Self::text with no gaps and no overlaps.
- `.effective_wrap_width(&self) -> f32` — The wrap with, with a small margin in some cases.
- `.font_height(&self, fonts: &mut FontsView<'_>) -> f32` — The height of the tallest font used in the job.
- `.format_at_byte(&self, byte_idx: ByteIndex) -> &TextFormat` — The TextFormat of the section containing the character starting at the given byte index.
- `.is_empty(&self) -> bool`
- `.simple(text: String, font_id: FontId, color: Color32, wrap_width: f32) -> Self` — Break on \n and at the given wrap width.
- `.simple_format(text: String, format: TextFormat) -> Self` — Break on \n
- `.simple_singleline(text: String, font_id: FontId, color: Color32) -> Self` — Does not break on \n, but shows the replacement character instead.
- `.single_section(text: String, format: TextFormat) -> Self`

**Galley**
- `.begin(&self) -> CCursor` — Cursor to the first character.
- `.clamp_cursor(&self, cursor: &CCursor) -> CCursor`
- `.concat(job: Arc<LayoutJob>, galleys: &[Arc<Self>], pixels_per_point: f32) -> Self` — Append each galley under the previous one.
- `.cursor_begin_of_paragraph(&self, cursor: &CCursor) -> CCursor`
- `.cursor_begin_of_row(&self, cursor: &CCursor) -> CCursor`
- `.cursor_down_one_row(&self, cursor: &CCursor, h_pos: Option<f32>,) -> (CCursor, Option<f32>)`
- `.cursor_end_of_paragraph(&self, cursor: &CCursor) -> CCursor`
- `.cursor_end_of_row(&self, cursor: &CCursor) -> CCursor`
- `.cursor_from_pos(&self, pos: Vec2) -> CCursor` — Cursor at the given position within the galley.
- `.cursor_left_one_character(&self, cursor: &CCursor) -> CCursor`
- `.cursor_right_one_character(&self, cursor: &CCursor) -> CCursor`
- `.cursor_up_one_row(&self, cursor: &CCursor, h_pos: Option<f32>,) -> (CCursor, Option<f32>)`
- `.end(&self) -> CCursor` — Cursor to one-past last character.
- `.intrinsic_size(&self) -> Vec2` — This is the size that a non-wrapped, non-truncated, non-justified version of the text would have.
- `.is_empty(&self) -> bool`
- `.layout_from_cursor(&self, cursor: CCursor) -> LayoutCursor`
- `.pos_from_cursor(&self, cursor: CCursor) -> Rect` — Returns a 0-width Rect.
- `.pos_from_layout_cursor(&self, layout_cursor: &LayoutCursor) -> Rect` — Returns a 0-width Rect.
- `.size(&self) -> Vec2`
- `.text(&self) -> &str` — The full, non-elided text of the input job.

**GalleyCache**
- `.flush_cache(&mut self)` — Must be called once per frame to clear the Galley cache.
- `.num_galleys_in_cache(&self) -> usize`

**Row**
- `.char_at(&self, desired_x: f32) -> CharIndex` — Closest char at the desired x coordinate in row-relative coordinates.
- `.char_count_excluding_newline(&self) -> CharIndex` — Excludes the implicit \n after the Row, if any.
- `.height(&self) -> f32`
- `.text(&self) -> String` — The text on this row, excluding the implicit \n if any.
- `.x_offset(&self, column: CharIndex) -> f32`

**PlacedRow**
- `.char_count_including_newline(&self) -> CharIndex` — Includes the implicit \n after the PlacedRow, if any.
- `.max_y(&self) -> f32`
- `.min_y(&self) -> f32`
- `.rect(&self) -> Rect` — Logical bounding rectangle on font heights etc.
- `.rect_without_leading_space(&self) -> Rect` — Same as Self::rect but excluding the LayoutSection::leading_space.

**Paragraph**
- `.from_section_index(section_index_at_start: u32) -> Self`

**Element**
- `.is_temp(&self) -> bool`

**Font**
- `.characters(&mut self) -> &BTreeMap<char, Vec<String>>` — All supported characters, and in which font they are available in.
- `.glyph_width(&mut self, c: char, font_size: f32) -> f32` — Width of this character in points, at the font's default variation location.
- `.has_glyph(&mut self, c: char) -> bool` — Can we display this glyph?
- `.has_glyphs(&mut self, s: &str) -> bool` — Can we display all the glyphs in this text?
- `.preload_characters(&mut self, s: &str)`
- `.styled_metrics(&self, pixels_per_point: f32, font_size: f32, coords: &VariationCoords,) -> StyledMetrics`

**FontFace**
- `.allocate_glyph(&mut self, atlas: &mut TextureAtlas, metrics: &StyledMetrics, shaped: &ShapedGlyph,) -> (GlyphAllocation, i32)`
- `.new(options: TextOptions, name: String, font_data: Blob, index: u32, tweak: FontTweak,) -> Result<Self, Box<dyn std::error::Error>>`
- `.styled_metrics(&self, pixels_per_point: f32, font_size: f32, coords: &VariationCoords,) -> StyledMetrics`

**FontData**
- `.from_owned(font: Vec<u8>) -> Self`
- `.from_static(font: &'static [u8]) -> Self`
- `.tweak(self, tweak: FontTweak) -> Self`

**FontDefinitions**
- `.builtin_font_names() -> &'static [&'static str]` — List of all the builtin font names used by epaint.
- `.builtin_font_names() -> &'static [&'static str]` — List of all the builtin font names used by epaint.
- `.empty() -> Self` — No fonts.

**FontId**
- `.monospace(size: f32) -> Self`
- `.new(size: f32, family: FontFamily) -> Self`
- `.proportional(size: f32) -> Self`

**FontInsert**
- `.new(name: &str, data: FontData, families: Vec<InsertFontFamily>) -> Self`

**FontSelection**
- `.resolve(self, style: &Style) -> FontId` — Resolve to a FontId.
- `.resolve_with_fallback(self, style: &Style, fallback: Self) -> FontId` — Resolve with a final fallback.

**Fonts**
- `.begin_pass(&mut self, options: TextOptions)` — Call at the start of each frame with the latest known TextOptions.
- `.definitions(&self) -> &FontDefinitions`
- `.font_atlas_fill_ratio(&self) -> f32` — How full is the font atlas?
- `.font_image_delta(&mut self) -> Option<crate::ImageDelta>` — Call at the end of each frame (before painting) to get the change to the font texture since last call.
- `.font_image_size(&self) -> [usize; 2]` — Current size of the font image.
- `.has_glyph(&mut self, font_id: &FontId, c: char) -> bool` — Can we display this glyph?
- `.has_glyphs(&mut self, font_id: &FontId, s: &str) -> bool` — Can we display all the glyphs in this text?
- `.image(&self) -> crate::ColorImage` — The full font atlas image.
- `.new(options: TextOptions, definitions: FontDefinitions) -> Self` — Create a new Fonts for text layout.
- `.num_galleys_in_cache(&self) -> usize`
- `.options(&self) -> &TextOptions`
- `.texture_atlas(&self) -> &TextureAtlas` — The font atlas.
- `.with_pixels_per_point(&mut self, pixels_per_point: f32) -> FontsView<'_>` — Returns a FontsView with the given pixels_per_point that can be used to do text layout.

**FontsImpl**
- `.font(&mut self, family: &FontFamily) -> Font<'_>` — Get the right font implementation from FontFamily.
- `.new(options: TextOptions, definitions: FontDefinitions) -> Self` — Create a new FontsImpl for text layout.
- `.options(&self) -> &TextOptions`
- `.return_shape_buffer(&mut self, buffer: harfrust::UnicodeBuffer)` — Return a shaping buffer for reuse.
- `.take_shape_buffer(&mut self) -> harfrust::UnicodeBuffer` — Take the recycled shaping buffer (or create a new one if already taken).

**FontsView**
- `.definitions(&self) -> &FontDefinitions`
- `.families(&self) -> Vec<FontFamily>` — List of all known font families.
- `.font_atlas_fill_ratio(&self) -> f32` — How full is the font atlas?
- `.font_image_size(&self) -> [usize; 2]` — Current size of the font image.
- `.glyph_width(&mut self, font_id: &FontId, c: char) -> f32` — Width of this character in points.
- `.has_glyph(&mut self, font_id: &FontId, c: char) -> bool` — Can we display this glyph?
- `.has_glyphs(&mut self, font_id: &FontId, s: &str) -> bool` — Can we display all the glyphs in this text?
- `.image(&self) -> crate::ColorImage` — The full font atlas image.
- `.layout(&mut self, text: String, font_id: FontId, color: crate::Color32, wrap_width: f32,) -> Arc<Galley>` — Will wrap text at the given width and line break at \n.
- `.layout_delayed_color(&mut self, text: String, font_id: FontId, wrap_width: f32,) -> Arc<Galley>` — Like Self::layout, made for when you want to pick a color for the text later.
- `.layout_job(&mut self, job: LayoutJob) -> Arc<Galley>` — Layout some text.
- `.layout_no_wrap(&mut self, text: String, font_id: FontId, color: crate::Color32,) -> Arc<Galley>` — Will line break at \n.
- `.num_galleys_in_cache(&self) -> usize`
- `.options(&self) -> &TextOptions`
- `.row_height(&mut self, font_id: &FontId) -> f32` — Height of one row of text in points.

**FontColorTransferFunction**
- `.alpha_from_coverage(self, coverage: f32) -> f32` — Convert coverage to alpha.
- `.color_from_coverage(self, coverage: f32) -> Color32`
- `.to_atlas_color(self, input_color: Color32) -> Color32` — How to convert a white color written by the font rasterizer into a color to be written into the font atlas.
- `.to_gamma(self) -> f32` — Convert this into the closest gamma exponent

**Glyph**
- `.logical_rect(&self) -> Rect` — Same y range for all characters with the same TextFormat.
- `.max_x(&self) -> f32`
- `.size(&self) -> Vec2`

**VariationCoords**
- `.clear(&mut self)`
- `.new<T: IntoTag>(values: impl IntoIterator<Item = (T, f32)>) -> Self` — Create a list of variation coordinates from a sequence of (tag, value) pairs.
- `.push(&mut self, tag: impl IntoTag, coord: f32)` — Add a variation coordinate to the list.
- `.remove(&mut self, index: usize)` — Remove the coordinate at the given index.

**SubpixelBin**
- `.as_float(&self) -> f32`

**PointScale**
- `.floor_to_pixel(&self, point: f32) -> f32`
- `.new(pixels_per_point: f32) -> Self`
- `.pixels_per_point(&self) -> f32`
- `.round_to_pixel(&self, point: f32) -> f32`

## Atoms — atomic widget layout

**Atom**
- `.custom(id: Id, size: impl Into<Vec2>) -> Self` — Create an AtomKind::Empty with a specific size.
- `.grow() -> Self` — Create an empty Atom marked as grow.
- `.into_sized(self, ui: &Ui, mut available_size: Vec2, mut wrap_mode: Option<TextWrapMode>, fallback_font: FontSelection,) -> SizedAtom<'a>` — Turn this into a SizedAtom.

**AtomKind**
- `.closure(func: impl FnOnce(&Ui, IntoSizedArgs) -> IntoSizedResult<'static> + 'a) -> Self` — See Self::Closure
- `.image(image: impl Into<Image<'a>>) -> Self` — See Self::Image
- `.into_sized(self, ui: &Ui, IntoSizedArgs { available_size, wrap_mode, fallback_font, }: IntoSizedArgs,) -> IntoSizedResult<'a>` — Turn this AtomKind into a SizedAtomKind.
- `.text(text: impl Into<WidgetText>) -> Self` — See Self::Text

**Atoms**
- `.any_shrink(&self) -> bool` — Do any of the atoms have shrink set to true?
- `.extend_left(&mut self, mut atoms: Self)` — Extend the list of atoms by prepending more atoms to the left side.
- `.extend_right(&mut self, atoms: Self)` — Extend the list of atoms by appending more atoms to the right side.
- `.iter_images(&self) -> impl Iterator<Item = &Image<'a>>`
- `.iter_images_mut(&mut self) -> impl Iterator<Item = &mut Image<'a>>`
- `.iter_kinds(&self) -> impl Iterator<Item = &AtomKind<'a>>`
- `.iter_kinds_mut(&mut self) -> impl Iterator<Item = &mut AtomKind<'a>>`
- `.iter_texts(&self) -> impl Iterator<Item = &WidgetText> + use<'_, 'a>`
- `.iter_texts_mut(&mut self) -> impl Iterator<Item = &mut WidgetText> + use<'a, '_>`
- `.map_atoms(&mut self, mut f: impl FnMut(Atom<'a>) -> Atom<'a>)`
- `.map_images<F>(&mut self, mut f: F) where F: FnMut(Image<'a>) -> Image<'a>,`
- `.map_kind<F>(&mut self, mut f: F) where F: FnMut(AtomKind<'a>) -> AtomKind<'a>,`
- `.map_texts<F>(&mut self, mut f: F) where F: FnMut(WidgetText) -> WidgetText,`
- `.new(atoms: impl IntoAtoms<'a>) -> Self`
- `.push_left(&mut self, atom: impl Into<Atom<'a>>)` — Insert a new Atom at the beginning of the list (left side).
- `.push_right(&mut self, atom: impl Into<Atom<'a>>)` — Insert a new Atom at the end of the list (right side).
- `.text(&self) -> Option<Cow<'_, str>>` — Concatenate and return the text contents.

**AtomLayout**
- `.align2(mut self, align2: Align2) -> Self` — Set the Align2.
- `.allocate(self, ui: &mut Ui) -> AllocatedAtomLayout<'a>` — Calculate sizes, create Galleys and allocate a Response.
- `.fallback_font(mut self, font: impl Into<FontSelection>) -> Self` — Set the fallback (default) font.
- `.fallback_text_color(mut self, color: Color32) -> Self` — Set the fallback (default) text color.
- `.frame(mut self, frame: Frame) -> Self` — Set the Frame.
- `.gap(mut self, gap: f32) -> Self` — Set the gap between atoms.
- `.id(mut self, id: Id) -> Self` — Set the Id used to allocate a Response.
- `.max_height(mut self, height: f32) -> Self` — Set the maximum height of the Widget.
- `.max_size(mut self, size: Vec2) -> Self` — Set the maximum size of the Widget.
- `.max_width(mut self, width: f32) -> Self` — Set the maximum width of the Widget.
- `.min_size(mut self, size: Vec2) -> Self` — Set the minimum size of the Widget.
- `.new(atoms: impl IntoAtoms<'a>) -> Self`
- `.selectable(mut self, selectable: bool) -> Self` — Make the text in this layout selectable with the mouse.
- `.sense(mut self, sense: Sense) -> Self` — Set the Sense used when allocating the Response.
- `.show(self, ui: &mut Ui) -> AtomLayoutResponse` — AtomLayout::allocate and AllocatedAtomLayout::paint in one go.
- `.wrap_mode(mut self, wrap_mode: TextWrapMode) -> Self` — Set the TextWrapMode for the crate::Atom marked as shrink.

**AllocatedAtomLayout**
- `.iter_images(&self) -> impl Iterator<Item = &Image<'atom>>`
- `.iter_images_mut(&mut self) -> impl Iterator<Item = &mut Image<'atom>>`
- `.iter_kinds(&self) -> impl Iterator<Item = &SizedAtomKind<'atom>>`
- `.iter_kinds_mut(&mut self) -> impl Iterator<Item = &mut SizedAtomKind<'atom>>`
- `.iter_texts(&self) -> impl Iterator<Item = &Arc<Galley>> + use<'atom, '_>`
- `.iter_texts_mut(&mut self) -> impl Iterator<Item = &mut Arc<Galley>> + use<'atom, '_>`
- `.map_images<F>(&mut self, mut f: F) where F: FnMut(Image<'atom>) -> Image<'atom>,`
- `.map_kind<F>(&mut self, mut f: F) where F: FnMut(SizedAtomKind<'atom>) -> SizedAtomKind<'atom>,`
- `.paint(self, ui: &Ui) -> AtomLayoutResponse` — Paint the Frame and individual crate::Atoms.

**SizedAtom**
- `.is_grow(&self) -> bool` — Was this crate::Atom marked as grow?

**SizedAtomKind**
- `.size(&self) -> Vec2` — Get the calculated size.

## Textures & images

**TextureHandle**
- `.aspect_ratio(&self) -> f32` — width / height
- `.byte_size(&self) -> usize` — width x height x bytes_per_pixel
- `.id(&self) -> TextureId`
- `.name(&self) -> String` — Debug-name.
- `.new(tex_mngr: Arc<RwLock<TextureManager>>, id: TextureId) -> Self` — If you are using egui, use egui::Context::load_texture instead.
- `.set(&mut self, image: impl Into<ImageData>, options: TextureOptions)` — Assign a new image to an existing texture.
- `.set_partial(&mut self, pos: [usize; 2], image: impl Into<ImageData>, options: TextureOptions,)` — Assign a new image to a subregion of the whole texture.
- `.size(&self) -> [usize; 2]` — width x height
- `.size_vec2(&self) -> crate::Vec2` — width x height

**TextureManager**
- `.alloc(&mut self, name: String, image: ImageData, options: TextureOptions) -> TextureId` — Allocate a new texture.
- `.allocated(&self) -> impl ExactSizeIterator<Item = (&TextureId, &TextureMeta)>` — Get meta-data about all allocated textures in some arbitrary order.
- `.free(&mut self, id: TextureId)` — Free an existing texture.
- `.meta(&self, id: TextureId) -> Option<&TextureMeta>` — Get meta-data about a specific texture.
- `.num_allocated(&self) -> usize` — Total number of allocated textures.
- `.retain(&mut self, id: TextureId)` — Increase the retain-count of the given texture.
- `.set(&mut self, id: TextureId, delta: ImageDelta)` — Assign a new image to an existing texture, or update a region of it.
- `.take_delta(&mut self) -> TexturesDelta` — Take and reset changes since last frame.

**TextureAtlas**
- `.allocate(&mut self, (w, h): (usize, usize)) -> ((usize, usize), &mut ColorImage)` — Returns the coordinates of where the rect ended up, and invalidates the region.
- `.fill_ratio(&self) -> f32` — When this get high, it might be time to clear and start over!
- `.image(&self) -> &ColorImage` — The full font atlas image.
- `.new(size: [usize; 2], options: TextOptions) -> Self`
- `.options(&self) -> &TextOptions`
- `.prepared_discs(&self) -> Vec<PreparedDisc>` — Returns the locations and sizes of pre-rasterized discs (filled circles) in this atlas.
- `.size(&self) -> [usize; 2]`
- `.take_delta(&mut self) -> Option<ImageDelta>` — Call to get the change to the image since last call.
- `.texture_options() -> crate::textures::TextureOptions` — The texture options suitable for a font texture

**TextureMeta**
- `.bytes_used(&self) -> usize` — Size in bytes. width x height x Self::bytes_per_pixel.

**TextureOptions**
- `.with_mipmap_mode(self, mipmap_mode: Option<TextureFilter>) -> Self`

**TexturePoll**
- `.is_pending(&self) -> bool`
- `.is_ready(&self) -> bool`
- `.size(&self) -> Option<Vec2>` — Point size of the original SVG, or the size of the image in texels.
- `.texture_id(&self) -> Option<TextureId>`

**TexturesDelta**
- `.append(&mut self, mut newer: Self)`
- `.clear(&mut self)`
- `.is_empty(&self) -> bool`

**SizedTexture**
- `.from_handle(handle: &TextureHandle) -> Self` — Fetch the [id]SizedTexture::id and [size]SizedTexture::size from a TextureHandle.
- `.new(id: impl Into<TextureId>, size: impl Into<Vec2>) -> Self` — Create a SizedTexture from a texture id with a specific size.

**ColorImage**
- `.as_raw(&self) -> &[u8]` — A view of the underlying data as &[u8]
- `.as_raw_mut(&mut self) -> &mut [u8]` — A view of the underlying data as &mut [u8]
- `.example() -> Self` — An example color image, useful for tests.
- `.filled(size: [usize; 2], color: Color32) -> Self` — Create an image filled with the given color.
- `.from_gray(size: [usize; 2], gray: &[u8]) -> Self` — Create a ColorImage from flat opaque gray data.
- `.from_gray_iter(size: [usize; 2], gray_iter: impl Iterator<Item = u8>) -> Self` — Alternative method to from_gray.
- `.from_rgb(size: [usize; 2], rgb: &[u8]) -> Self` — Create a ColorImage from flat RGB data.
- `.from_rgba_premultiplied(size: [usize; 2], rgba: &[u8]) -> Self`
- `.from_rgba_unmultiplied(size: [usize; 2], rgba: &[u8]) -> Self` — Create a ColorImage from flat un-multiplied RGBA data.
- `.height(&self) -> usize`
- `.new(size: [usize; 2], pixels: Vec<Color32>) -> Self` — Create an image filled with the given color.
- `.region(&self, region: &emath::Rect, pixels_per_point: Option<f32>) -> Self` — Create a new image from a patch of the current image.
- `.region_by_pixels(&self, [x, y]: [usize; 2], [w, h]: [usize; 2]) -> Self` — Clone a sub-region as a new image.
- `.width(&self) -> usize`
- `.with_source_size(mut self, source_size: Vec2) -> Self` — Set the source size of e.g. the original SVG image.

## Loaders

**Loaders**
- `.end_pass(&self, pass_index: u64)` — The given pass has just ended.

**DefaultBytesLoader**
- `.insert(&self, uri: impl Into<Cow<'static, str>>, bytes: impl Into<Bytes>)`

**LoadError**
- `.byte_size(&self) -> usize` — Returns the (approximate) size of the error message in bytes.

**SizeHint**
- `.scale_by(self, factor: f32) -> Self` — Multiply size hint by a factor.

## Viewport / windowing output

**ViewportBuilder**
- `.patch(&mut self, new_vp_builder: Self) -> (Vec<ViewportCommand>, bool)` — Update this ViewportBuilder with a delta, returning a list of commands and a bool indicating if the window needs to be recreated.
- `.with_active(mut self, active: bool) -> Self` — Whether the window will be initially focused or not.
- `.with_always_on_top(self) -> Self` — This window is always on top
- `.with_app_id(mut self, app_id: impl Into<String>) -> Self` — On Wayland this sets the Application ID for the window.
- `.with_clamp_size_to_monitor_size(mut self, value: bool) -> Self` — Sets whether clamp the window's size to monitor's size.
- `.with_close_button(mut self, value: bool) -> Self` — Does not work on X11.
- `.with_decorations(mut self, decorations: bool) -> Self` — Sets whether the window should have a border, a title bar, etc.
- `.with_drag_and_drop(mut self, value: bool) -> Self` — On Windows: enable drag and drop support.
- `.with_fullscreen(mut self, fullscreen: bool) -> Self` — Sets whether the window should be put into fullscreen upon creation.
- `.with_fullsize_content_view(mut self, value: bool) -> Self` — macOS: Makes the window content appear behind the titlebar.
- `.with_has_shadow(mut self, has_shadow: bool) -> Self` — macOS: Set to false to make the window render without a drop shadow.
- `.with_icon(mut self, icon: impl Into<Arc<IconData>>) -> Self` — The application icon, e.g. in the Windows task bar or the alt-tab menu.
- `.with_inner_size(mut self, size: impl Into<Vec2>) -> Self` — Requests the window to be of specific dimensions.
- `.with_max_inner_size(mut self, size: impl Into<Vec2>) -> Self` — Sets the maximum dimensions a window can have.
- `.with_maximize_button(mut self, value: bool) -> Self` — Does not work on X11.
- `.with_maximized(mut self, maximized: bool) -> Self` — Request that the window is maximized upon creation.
- `.with_min_inner_size(mut self, size: impl Into<Vec2>) -> Self` — Sets the minimum dimensions a window can have.
- `.with_minimize_button(mut self, value: bool) -> Self` — Does not work on X11.
- `.with_monitor(mut self, index: usize) -> Self` — Place the window in borderless fullscreen on the monitor at index.
- `.with_mouse_passthrough(mut self, value: bool) -> Self` — On desktop: mouse clicks pass through the window, used for non-interactable overlays.
- `.with_movable_by_background(mut self, value: bool) -> Self` — macOS: Set to true to allow the window to be moved by dragging the background.
- `.with_override_redirect(mut self, value: bool) -> Self` — This sets the override-redirect flag.
- `.with_position(mut self, pos: impl Into<Pos2>) -> Self` — The initial "outer" position of the window, i.e. where the top-left corner of the frame/chrome should be.
- `.with_resizable(mut self, resizable: bool) -> Self` — Sets whether the window is resizable or not.
- `.with_taskbar(mut self, show: bool) -> Self` — windows: Whether show or hide the window icon in the taskbar.
- `.with_title(mut self, title: impl Into<String>) -> Self` — Sets the initial title of the window in the title bar.
- `.with_title_shown(mut self, title_shown: bool) -> Self` — macOS: Set to false to hide the window title.
- `.with_titlebar_buttons_shown(mut self, titlebar_buttons_shown: bool) -> Self` — macOS: Set to false to hide the titlebar button (close, minimize, maximize)
- `.with_titlebar_shown(mut self, shown: bool) -> Self` — macOS: Set to false to make the titlebar transparent, allowing the content to appear behind it.
- `.with_transparent(mut self, transparent: bool) -> Self` — Sets whether the background of the window should be transparent.
- `.with_visible(mut self, visible: bool) -> Self` — Sets whether the window will be initially visible or hidden.
- `.with_window_level(mut self, level: WindowLevel) -> Self` — Control if window is always-on-top, always-on-bottom, or neither.
- `.with_window_type(mut self, value: X11WindowType) -> Self` — This sets the window type.

**ViewportCommand**
- `.center_on_screen(ctx: &crate::Context) -> Option<Self>` — Construct a command to center the viewport on the monitor, if possible.
- `.requires_parent_repaint(&self) -> bool` — This command requires the parent viewport to repaint.

**ViewportId**
- `.from_hash_of(source: impl AsId) -> Self`

**ViewportIdPair**
- `.from_self_and_parent(this: ViewportId, parent: ViewportId) -> Self`

**ViewportInfo**
- `.close_requested(&self) -> bool` — This viewport has been told to close.
- `.take(&mut self) -> Self` — Helper: move Self::events, clone the other fields.
- `.ui(&self, ui: &mut crate::Ui)`
- `.visible(&self) -> Option<bool>` — Is the window considered visible for rendering purposes?

**ViewportInPixels**
- `.from_points(rect: &Rect, pixels_per_point: f32, screen_size_px: [u32; 2]) -> Self` — Convert from ui points.

**ViewportOutput**
- `.append(&mut self, newer: Self)` — Add on new output.

**ViewportRepaintInfo**
- `.requested_immediate_repaint_prev_pass(&self) -> bool`

**FullOutput**
- `.append(&mut self, newer: Self)` — Add on new output.

**PlatformOutput**
- `.append(&mut self, newer: Self)` — Add on new output.
- `.events_description(&self) -> String` — This can be used by a text-to-speech system to describe the events (if any).
- `.requested_discard(&self) -> bool` — Was crate::Context::request_discard called?
- `.take(&mut self) -> Self` — Take everything ephemeral (everything except cursor_icon and cursor_image currently)

**OutputEvent**
- `.widget_info(&self) -> &WidgetInfo`

**OpenUrl**
- `.new_tab(url: impl ToString) -> Self`
- `.same_tab(url: impl ToString) -> Self`

**IconData**
- `.is_empty(&self) -> bool`

**RepaintCause**
- `.new() -> Self` — Capture the file and line number of the call site.
- `.new_reason(reason: impl Into<Cow<'static, str>>) -> Self` — Capture the file and line number of the call site, as well as add a reason.

## Plugins / misc internals

**PluginHandle**
- `.dyn_plugin_mut(&mut self) -> &mut dyn Plugin`
- `.new<P: Plugin>(plugin: P) -> Arc<Mutex<Self>>`
- `.typed_plugin_mut<P: Plugin + 'static>(&mut self) -> &mut P`

**Plugins**
- `.add(&mut self, handle: Arc<Mutex<PluginHandle>>) -> bool` — Remember to call Plugin::setup on the plugin after adding it.
- `.get(&self, type_id: std::any::TypeId) -> Option<Arc<Mutex<PluginHandle>>>`
- `.ordered_plugins(&self) -> PluginsOrdered`

**PluginsOrdered**
- `.on_begin_pass(&self, ui: &mut Ui)`
- `.on_end_pass(&self, ui: &mut Ui)`
- `.on_input(&self, ctx: &Context, input: &mut RawInput)`
- `.on_output(&self, ctx: &Context, output: &mut FullOutput)`
- `.on_widget_under_pointer(&self, ctx: &Context, widget: &crate::WidgetRect)`

**TypedPluginHandle**
- `.lock(&self) -> TypedPluginGuard<'_, P>` — Lock the plugin for access.

**AnimationManager**
- `.animate_bool(&mut self, input: &InputState, animation_time: f32, id: Id, value: bool,) -> f32` — See crate::Context::animate_bool for documentation
- `.animate_value(&mut self, input: &InputState, animation_time: f32, id: Id, value: f32,) -> f32`

**DebugOptions**
- `.ui(&mut self, ui: &mut crate::Ui)`

**DebugRect**
- `.paint(self, painter: &Painter)`

**Tessellator**
- `.new(pixels_per_point: f32, options: TessellationOptions, font_tex_size: [usize; 2], prepared_discs: Vec<PreparedDisc>,) -> Self` — Create a new Tessellator.
- `.set_clip_rect(&mut self, clip_rect: Rect)` — Set the Rect to use for culling.
- `.tessellate_circle(&mut self, shape: CircleShape, out: &mut Mesh)` — Tessellate a single CircleShape into a Mesh.
- `.tessellate_clipped_shape(&mut self, clipped_shape: ClippedShape, out_primitives: &mut Vec<ClippedPrimitive>,)` — Tessellate a clipped shape into a list of primitives.
- `.tessellate_cubic_bezier(&mut self, cubic_shape: &CubicBezierShape, out: &mut Mesh)` — Tessellate a single CubicBezierShape into a Mesh.
- `.tessellate_ellipse(&mut self, shape: EllipseShape, out: &mut Mesh)` — Tessellate a single EllipseShape into a Mesh.
- `.tessellate_line_segment(&mut self, mut points: [Pos2; 2], stroke: impl Into<Stroke>, out: &mut Mesh,)` — Tessellate a line segment between the two points with the given stroke into a Mesh.
- `.tessellate_mesh(&self, mesh: &Mesh, out: &mut Mesh)` — Tessellate a single Mesh into a Mesh.
- `.tessellate_path(&mut self, path_shape: &PathShape, out: &mut Mesh)` — Tessellate a single PathShape into a Mesh.
- `.tessellate_quadratic_bezier(&mut self, quadratic_shape: &QuadraticBezierShape, out: &mut Mesh,)` — Tessellate a single QuadraticBezierShape into a Mesh.
- `.tessellate_rect(&mut self, rect_shape: &RectShape, out: &mut Mesh)` — Tessellate a single Rect into a Mesh.
- `.tessellate_shape(&mut self, shape: Shape, out: &mut Mesh)` — Tessellate a single Shape into a Mesh.
- `.tessellate_shapes(&mut self, mut shapes: Vec<ClippedShape>) -> Vec<ClippedPrimitive>` — Turns Shape:s into sets of triangles.
- `.tessellate_text(&mut self, text_shape: &TextShape, out: &mut Mesh)` — Tessellate a single TextShape into a Mesh. * text_shape: the text to tessellate. * out: triangles are appended to this.

**PaintStats**
- `.from_shapes(shapes: &[ClippedShape]) -> Self`
- `.with_clipped_primitives(mut self, clipped_primitives: &[crate::ClippedPrimitive],) -> Self`

**AllocInfo**
- `.format(&self, what: &str) -> String`
- `.from_galley(galley: &Galley) -> Self`
- `.from_mesh(mesh: &Mesh) -> Self`
- `.from_slice<T>(slice: &[T]) -> Self`
- `.megabytes(&self) -> String`
- `.num_allocs(&self) -> usize`
- `.num_bytes(&self) -> usize`
- `.num_elements(&self) -> usize`

**PaintCallbackInfo**
- `.clip_rect_in_pixels(&self) -> ViewportInPixels` — The "scissor" or "clip" rectangle.
- `.viewport_in_pixels(&self) -> ViewportInPixels` — The viewport rectangle.

**FrameDurations**
- `.all(&self) -> Iter<'_, Duration>`
- `.new(durations: Vec<Duration>) -> Self`

**FrameCache**
- `.evict_cache(&mut self)` — Must be called once per frame to clear the cache.
- `.get<Key>(&mut self, key: Key) -> &Value where Key: Copy + std::hash::Hash, Computer: ComputerMut<Key, Value>,` — Get from cache (if the same key was used last frame) or recompute and store in the cache.
- `.new(computer: Computer) -> Self`

**FramePublisher**
- `.evict_cache(&mut self)` — Must be called once per frame to clear the cache.
- `.get(&self, key: &Key) -> Option<&Value>` — Retrieve a value if it was published this or the previous frame.
- `.new() -> Self`
- `.set(&mut self, key: Key, value: Value)` — Publish the value.

**CacheStorage**
- `.cache<Cache: CacheTrait + Default>(&mut self) -> &mut Cache`
- `.update(&mut self)` — Call once per frame to evict cache.

**Options**
- `.begin_pass(&mut self, new_raw_input: &RawInput)`
- `.ui(&mut self, ui: &mut crate::Ui)` — Show the options in the ui.

**InputOptions**
- `.ui(&mut self, ui: &mut crate::Ui)` — Show the options in the ui.

**Order**
- `.allow_interaction(&self) -> bool`
- `.short_debug_format(&self) -> &'static str` — Short and readable summary

**LayerId**
- `.background() -> Self`
- `.debug() -> Self`
- `.new(order: Order, id: Id) -> Self`
- `.short_debug_format(&self) -> String` — Short and readable summary

**OperatingSystem**
- `.from_target_os() -> Self` — Uses the compile-time target_arch to identify the OS.
- `.from_user_agent(user_agent: &str) -> Self` — Helper: try to guess from the user-agent of a browser.
- `.is_mac(&self) -> bool` — Are we either macOS or iOS?

**UserData**
- `.new(user_info: impl Any + Send + Sync) -> Self` — You can also use Self::default.

**SurrenderFocusOn**
- `.ui(&mut self, ui: &mut crate::Ui)`

**ModifierNames**
- `.format(&self, modifiers: &Modifiers, is_mac: bool) -> String`

**RawKey**
- `.new<T: 'static>(id: Id) -> Self` — Create a new key for the given type.

**UvRect**
- `.is_nothing(&self) -> bool`

**LocationHash**
- `.new(location: &skrifa::instance::Location) -> Self`

**TypeId**
- `.of<T: Any + 'static>() -> Self`

**OrderedFloat**
- `.into_inner(self) -> T`

**ClippedShape**
- `.transform(&mut self, transform: emath::TSTransform)` — Transform (move/scale) the shape in-place.

**FlatteningParameters**
- `.from_curve(curve: &QuadraticBezierShape, tolerance: f32) -> Self`

**Path**
- `.add_circle(&mut self, center: Pos2, radius: f32)`
- `.add_line_loop(&mut self, points: &[Pos2])`
- `.add_line_segment(&mut self, points: [Pos2; 2])`
- `.add_open_points(&mut self, points: &[Pos2])`
- `.add_point(&mut self, pos: Pos2, normal: Vec2)`
- `.clear(&mut self)`
- `.fill(&mut self, feathering: f32, color: Color32, out: &mut Mesh)` — The path is taken to be closed (i.e. returning to the start again).
- `.fill_and_stroke(&mut self, feathering: f32, fill: Color32, stroke: &PathStroke, out: &mut Mesh,)` — The path is taken to be closed (i.e. returning to the start again).
- `.fill_with_uv(&mut self, feathering: f32, color: Color32, texture_id: TextureId, uv_from_pos: impl Fn(Pos2) -> Pos2, out: &mut Mesh,)` — Like Self::fill but with texturing.
- `.reserve(&mut self, additional: usize)`
- `.stroke(&mut self, feathering: f32, path_type: PathType, stroke: &PathStroke, out: &mut Mesh,)`
- `.stroke_closed(&mut self, feathering: f32, stroke: &PathStroke, out: &mut Mesh)` — A closed path (returning to the first point).
- `.stroke_open(&mut self, feathering: f32, stroke: &PathStroke, out: &mut Mesh)` — Open-ended.

**NumericColorSpace**
- `.toggle_button_ui(&mut self, ui: &mut Ui) -> crate::Response`

**Mutex**
- `.lock(&self) -> MutexGuard<'_, T>` — Try to acquire the lock.
- `.new(val: T) -> Self`

**RwLock**
- `.new(val: T) -> Self`
- `.read(&self) -> RwLockReadGuard<'_, T>` — Try to acquire read-access to the lock.
- `.write(&self) -> RwLockWriteGuard<'_, T>` — Try to acquire write-access to the lock.

**State**
- `.load(ctx: &Context, id: Id) -> Option<Self>`
- `.load(ctx: &Context, id: Id) -> Option<Self>`
- `.load(ctx: &Context, id: Id) -> Option<Self>`
- `.store(self, ctx: &Context, id: Id)`
- `.store(self, ctx: &Context, id: Id)`
- `.store(self, ctx: &Context, id: Id)`
- `.velocity(&self) -> Vec2` — Get the current kinetic scrolling velocity.

**Prepared**
- `.allocate_space(&self, ui: &mut Ui) -> Response` — Allocate the space that was used by Self::content_ui.
- `.end(self, ui: &mut Ui) -> Response` — Convenience for calling Self::allocate_space and Self::paint.
- `.paint(&self, ui: &Ui)` — Paint the frame.

**f64**
- `.lerp<R, T>(range: impl Into<RangeInclusive<R>>, t: T) -> R where T: Real + Mul<R, Output = R>, R: Copy + Add<R, Output = R>,` — Linear interpolation.

**ClosableTag**
- `.set_close(&self)` — Set close to true
- `.should_close(&self) -> bool` — Returns true if ClosableTag::set_close has been called.

## Free functions (module-level helpers)

**mod `crates/ecolor/src/hsva`**
- `hsv_from_rgb([r, g, b]: [f32; 3]) -> (f32, f32, f32)` — All ranges in 0-1, rgb is linear.
- `rgb_from_hsv((h, s, v): (f32, f32, f32)) -> [f32; 3]` — All ranges in 0-1, rgb is linear.

**mod `crates/ecolor/src/lib`**
- `gamma_from_linear(linear: f32) -> f32` — linear [0, 1] -> gamma [0, 1] (not clamped).
- `gamma_u8_from_linear_f32(l: f32) -> u8` — linear [0, 1] -> gamma [0, 255] (clamped).
- `linear_f32_from_gamma_u8(s: u8) -> f32` — gamma [0, 255] -> linear [0, 1].
- `linear_f32_from_linear_u8(a: u8) -> f32` — linear [0, 255] -> linear [0, 1].
- `linear_from_gamma(gamma: f32) -> f32` — gamma [0, 1] -> linear [0, 1] (not clamped).
- `linear_u8_from_linear_f32(a: f32) -> u8` — linear [0, 1] -> linear [0, 255] (clamped).
- `test_srgba_conversion()`
- `tint_color_towards(color: Color32, target: Color32) -> Color32` — Cheap and ugly.

**mod `callstack`**
- `capture() -> String` — Capture a callstack, skipping the frames that are not interesting.

**mod `containers/collapsing_header`**
- `paint_default_icon(ui: &mut Ui, openness: f32, response: &Response)` — Paint the arrow icon that indicated if the region is open or not

**mod `containers/menu`**
- `find_menu_root(ui: &Ui) -> &UiStack` — Find the root UiStack of the menu.
- `is_in_menu(ui: &Ui) -> bool` — Is this Ui part of a menu?
- `menu_style(style: &mut Style)` — Apply a menu style to the Style.

**mod `containers/resize`**
- `paint_resize_corner(ui: &Ui, response: &Response)`
- `paint_resize_corner_with_style(ui: &Ui, rect: &Rect, color: impl Into<Color32>, corner: Align2,)`

**mod `debug_text`**
- `print(ctx: &Context, text: impl Into<WidgetText>)` — Print this text next to the cursor at the end of the pass.

**mod `gui_zoom`**
- `zoom_in(ctx: &Context)` — Make everything larger by increasing Context::zoom_factor.
- `zoom_menu_buttons(ui: &mut Ui)` — Show buttons for zooming the ui.
- `zoom_out(ctx: &Context)` — Make everything smaller by decreasing Context::zoom_factor.

**mod `hit_test`**
- `hit_test(widgets: &WidgetRects, layer_order: &[LayerId], layer_to_global: &HashMap<LayerId, TSTransform>, pos: Pos2, search_radius: f32,) -> WidgetHits` — Find the top or closest widgets to the given position, none which is closer than search_radius.

**mod `introspection`**
- `font_family_ui(ui: &mut Ui, font_family: &mut FontFamily)`
- `font_id_ui(ui: &mut Ui, font_id: &mut FontId)`

**mod `lib`**
- `__run_test_ctx(mut run_ui: impl FnMut(&Context))` — For use in tests; especially doctests.
- `__run_test_ui(mut add_contents: impl FnMut(&mut Ui))` — For use in tests; especially doctests.
- `accesskit_root_id() -> Id`
- `warn_if_debug_build(ui: &mut crate::Ui)` — Helper function that adds a label when compiling with debug assertions enabled.

**mod `style`**
- `default_text_styles() -> BTreeMap<TextStyle, FontId>` — The default text styles of the default egui theme.

**mod `text_selection/accesskit_text`**
- `update_accesskit_for_text_widget(ctx: &Context, widget_id: Id, cursor_range: Option<CCursorRange>, role: accesskit::Role, global_from_galley: TSTransform, galley: &Galley,)` — Update accesskit with the current text state.

**mod `text_selection/text_cursor_state`**
- `byte_index_from_char_index(s: &str, char_index: CharIndex) -> ByteIndex`
- `ccursor_next_word(text: &str, ccursor: CCursor) -> CCursor`
- `ccursor_previous_word(text: &str, ccursor: CCursor) -> CCursor`
- `char_index_from_byte_index(input: &str, byte_index: ByteIndex) -> CharIndex`
- `cursor_rect(galley: &Galley, cursor: &CCursor, row_height: f32) -> Rect` — The thin rectangle of one end of the selection, e.g. the primary cursor, in local galley coordinates.
- `find_line_start(text: &str, current_index: CCursor) -> CCursor` — Accepts and returns character offset (NOT byte offset!).
- `is_word_char(c: char) -> bool`
- `slice_char_range(s: &str, char_range: std::ops::Range<CharIndex>) -> &str`

**mod `text_selection/visuals`**
- `paint_cursor_end(painter: &Painter, visuals: &Visuals, cursor_rect: Rect)` — Paint one end of the selection, e.g. the primary cursor.
- `paint_text_cursor(ui: &Ui, painter: &Painter, primary_cursor_rect: Rect, time_since_last_interaction: f64,)` — Paint one end of the selection, e.g. the primary cursor, with blinking (if enabled).
- `paint_text_selection(galley: &mut Arc<Galley>, visuals: &Visuals, cursor_range: &CCursorRange, mut new_vertex_indices: Option<&mut Vec<RowVertexIndices>>,)` — Adds text selection rectangles to the galley.

**mod `widgets/color_picker`**
- `color_edit_button_hsva(ui: &mut Ui, hsva: &mut Hsva, alpha: Alpha) -> Response`
- `color_edit_button_rgb(ui: &mut Ui, rgb: &mut [f32; 3]) -> Response` — Shows a button with the given color.
- `color_edit_button_rgba(ui: &mut Ui, rgba: &mut Rgba, alpha: Alpha) -> Response` — Shows a button with the given color.
- `color_edit_button_srgb(ui: &mut Ui, srgb: &mut [u8; 3]) -> Response` — Shows a button with the given color.
- `color_edit_button_srgba(ui: &mut Ui, srgba: &mut Color32, alpha: Alpha) -> Response` — Shows a button with the given color.
- `color_picker_color32(ui: &mut Ui, srgba: &mut Color32, alpha: Alpha) -> bool` — Shows a color picker where the user can change the given Color32 color.
- `color_picker_hsva_2d(ui: &mut Ui, hsva: &mut Hsva, alpha: Alpha) -> bool` — Shows a color picker where the user can change the given Hsva color.
- `show_color(ui: &mut Ui, color: impl Into<Color32>, desired_size: Vec2) -> Response` — Show a color with background checkers to demonstrate transparency (if any).
- `show_color_at(painter: &Painter, color: Color32, rect: Rect)` — Show a color with background checkers to demonstrate transparency (if any).

**mod `widgets/image`**
- `decode_animated_image_uri(uri: &str) -> Result<(&str, usize), String>` — Extracts uri and frame index # Errors Will return Err if uri does not match pattern {uri}-{frame_index}
- `has_gif_magic_header(bytes: &[u8]) -> bool` — Checks if bytes are gifs
- `has_webp_header(bytes: &[u8]) -> bool` — Checks if bytes are webp
- `paint_texture_at(painter: &Painter, rect: Rect, options: &ImageOptions, texture: &SizedTexture,)`
- `paint_texture_load_result(ui: &Ui, tlr: &TextureLoadResult, rect: Rect, show_loading_spinner: Option<bool>, options: &ImageOptions, alt_text: Option<&str>,)`
- `texture_load_result_response(source: &ImageSource<'_>, tlr: &TextureLoadResult, response: Response,) -> Response` — Attach tooltips like "Loading…" or "Failed loading: …".

**mod `widgets/mod`**
- `global_theme_preference_buttons(ui: &mut Ui)` — Show larger buttons for switching between light and dark mode (globally).
- `global_theme_preference_switch(ui: &mut Ui)` — Show a small button to switch to/from dark/light mode (globally).
- `reset_button<T: Default + PartialEq>(ui: &mut Ui, value: &mut T, text: &str)` — Show a button to reset a value to its default.
- `reset_button_with<T: PartialEq>(ui: &mut Ui, value: &mut T, text: &str, reset_value: T)` — Show a button to reset a value to its default.

**mod `crates/emath/src/align`**
- `center_size_in_rect(size: Vec2, frame: Rect) -> Rect` — Allocates a rectangle of the specified size inside the frame rectangle around of its center.

**mod `crates/emath/src/easing`**
- `back_in(t: f32) -> f32` — <https://easings.net/#easeInBack>
- `back_in_out(t: f32) -> f32` — <https://easings.net/#easeInOutBack>
- `back_out(t: f32) -> f32` — <https://easings.net/#easeOutBack>
- `bounce_in(t: f32) -> f32` — <https://easings.net/#easeInBounce>
- `bounce_in_out(t: f32) -> f32` — <https://easings.net/#easeInOutBounce>
- `bounce_out(t: f32) -> f32` — <https://easings.net/#easeOutBounce>
- `circular_in(t: f32) -> f32` — <https://easings.net/#easeInCirc>
- `circular_in_out(t: f32) -> f32` — <https://easings.net/#easeInOutCirc>
- `circular_out(t: f32) -> f32` — <https://easings.net/#easeOutCirc>
- `cubic_in(t: f32) -> f32` — <https://easings.net/#easeInCubic>
- `cubic_in_out(t: f32) -> f32` — <https://easings.net/#easeInOutCubic>
- `cubic_out(t: f32) -> f32` — <https://easings.net/#easeOutCubic>
- `exponential_in(t: f32) -> f32` — <https://easings.net/#easeInExpo>
- `exponential_in_out(t: f32) -> f32` — <https://easings.net/#easeInOutExpo>
- `exponential_out(t: f32) -> f32` — <https://easings.net/#easeOutExpo>
- `linear(t: f32) -> f32` — No easing, just y = x
- `quadratic_in(t: f32) -> f32` — <https://easings.net/#easeInQuad>
- `quadratic_in_out(t: f32) -> f32` — <https://easings.net/#easeInOutQuad>
- `quadratic_out(t: f32) -> f32` — <https://easings.net/#easeOutQuad>
- `sin_in(t: f32) -> f32` — <https://easings.net/#easeInSine>
- `sin_in_out(t: f32) -> f32` — <https://easings.net/#easeInOutSine>
- `sin_out(t: f32) -> f32` — <https://easings.net/#easeOuSine>

**mod `crates/emath/src/lib`**
- `almost_equal(a: f32, b: f32, epsilon: f32) -> bool` — Return true when arguments are the same within some rounding error.
- `ease_in_ease_out(t: f32) -> f32` — Ease in, ease out.
- `exponential_smooth_factor(reach_this_fraction: f32, in_this_many_seconds: f32, dt: f32,) -> f32` — Calculate a lerp-factor for exponential smoothing using a time step.
- `fast_midpoint<R>(a: R, b: R) -> R where R: Copy + Add<R, Output = R> + Div<R, Output = R> + One,` — This is a faster version of f32::midpoint which doesn't handle overflow.
- `format_with_decimals_in_range(value: f64, decimal_range: RangeInclusive<usize>) -> String` — Use as few decimals as possible to show the value accurately, but within the given range.
- `format_with_minimum_decimals(value: f64, decimals: usize) -> String`
- `interpolation_factor((start_time, end_time): (f64, f64), current_time: f64, dt: f32, easing: impl Fn(f32) -> f32,) -> f32` — If you have a value animating over time, how much towards its target do you need to move it this frame?
- `inverse_lerp<R>(range: RangeInclusive<R>, value: R) -> Option<R> where R: Copy + PartialEq + Sub<R, Output = R> + Div<R, Output = R>,` — Where in the range is this value? Returns 0-1 if within the range.
- `normalized_angle(mut angle: f32) -> f32` — Wrap angle to [-PI, PI] range.
- `remap<T>(x: T, from: impl Into<RangeInclusive<T>>, to: impl Into<RangeInclusive<T>>) -> T where T: Real,` — Linearly remap a value from one range to another, so that when x == from.start() returns to.start() and when x == from.end() returns to.end().
- `remap_clamp<T>(x: T, from: impl Into<RangeInclusive<T>>, to: impl Into<RangeInclusive<T>>,) -> T where T: Real,` — Like remap, but also clamps the value so that the returned value is always in the to range.
- `round_to_decimals(value: f64, decimal_places: usize) -> f64` — Round a value to the given number of decimal places.

**mod `crates/emath/src/pos2`**
- `pos2(x: f32, y: f32) -> Pos2` — pos2(x, y) == Pos2::new(x, y)

**mod `crates/emath/src/smart_aim`**
- `best_in_range_f64(min: f64, max: f64) -> f64` — Find the "simplest" number in a closed range [min, max], i.e. the one with the fewest decimal digits.

**mod `crates/emath/src/vec2`**
- `vec2(x: f32, y: f32) -> Vec2` — vec2(x, y) == Vec2::new(x, y)

**mod `crates/epaint/src/shape_transform`**
- `adjust_colors(shape: &mut Shape, adjust_color: impl Fn(&mut Color32) + Send + Sync + Copy + 'static,)` — Remember to handle Color32::PLACEHOLDER specially!

**mod `crates/epaint/src/tessellator`**
- `add_circle_quadrant(path: &mut Vec<Pos2>, center: Pos2, radius: f32, quadrant: f32)` — Add one quadrant of a circle
- `rounded_rectangle(path: &mut Vec<Pos2>, rect: Rect, cr: CornerRadiusF32)` — overwrites existing points

**mod `crates/epaint/src/text/index`**
- `saturating_add(self, rhs: usize) -> Self` — Saturating integer addition.
- `saturating_sub(self, rhs: usize) -> Self` — Saturating integer subtraction.

**mod `crates/epaint/src/text/text_layout`**
- `layout(fonts: &mut FontsImpl, pixels_per_point: f32, job: Arc<LayoutJob>) -> Galley` — Layout text into a Galley.

**mod `crates/epaint/src/util/mod`**
- `hash(value: impl std::hash::Hash) -> u64` — Hash the given value with a predictable hasher.
- `hash_with(value: impl std::hash::Hash, mut hasher: impl std::hash::Hasher) -> u64` — Hash the given value with the given hasher.