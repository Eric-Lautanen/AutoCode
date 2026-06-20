# egui 0.34 Migration Notes

## Key changes from 0.33

- `egui::Context` methods now return `Id` from `make_persistent_id` instead of `usize`
- `egui::Slider` now uses `Into<f64>` instead of `Into<f32>` for range bounds
- `egui::plot::Plot` renamed `PlotItem` trait - use `PlotItem::shapes()` instead of `PlotItem::values()`
- `egui::Panel` - `frame` parameter is now `Option<Frame>` instead of `Frame`
- `egui::TopBottomPanel::top` signature changed - `resizable` parameter replaced with `Resize` struct

## Migration pattern

```rust
// Before (0.33)
egui::TopBottomPanel::top("panel").resizable(true).show(ctx, |ui| {});

// After (0.34)
egui::TopBottomPanel::top("panel")
    .resizable(egui::panel::Resize::default().enabled(true))
    .show(ctx, |ui| {});
```

## Breaking: Default padding reduced

Panel content padding is now 6.0 instead of 8.0. Use `.frame()` to restore.

```rust
egui::Frame::group(&style).inner_margin(egui::Margin::symmetric(8.0, 8.0))
```
