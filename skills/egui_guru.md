```markdown
---
name: egui_guru
description: Use when building desktop or web GUIs using egui/eframe. Covers immediate mode patterns, state management, custom widgets, layout, performance optimization, and advanced ecosystem integration. Load when asked about egui UI, styling, app structure, or complex egui workflows.
---

# Egui Guru

## Overview
egui (pronounced "e-gooey") is a simple, fast, and highly portable immediate-mode GUI library for Rust that runs natively, on the web, and in game engines 【turn0search30】. As of mid-2026, the latest stable versions are **egui 0.34.3** and **eframe 0.34**, with **egui_plot 0.35.0** for data visualization 【turn0search3】【turn0search4】. The library is in active development, with breaking changes between versions, so pinning versions and monitoring the [changelog](https://github.com/emilk/egui/blob/master/CHANGELOG.md) is essential 【turn0search25】.

Immediate-mode GUIs redraw the entire UI every frame, which simplifies state management and provides instant feedback, ideal for real-time applications like data visualization, tools, and game development 【turn0search5】【turn0search31】. While this can increase CPU usage, egui's design includes optimizations like clipping, lazy updates, and efficient rendering to mitigate this 【turn0search30】【turn0search34】.

## 📦 Ecosystem & Version Matrix
| Crate | Version | Purpose | Key Features |
|-------|---------|---------|--------------|
| `egui` | 0.34.3 | Core GUI library | Immediate-mode widgets, layout, styling |
| `eframe` | 0.34 | App framework | Web/native support, `App` trait, persistence |
| `egui_plot` | 0.35.0 | Data visualization | 2D plotting, real-time data, custom plots |
| `bevy_egui` | 0.35.1 | Bevy integration | Game engine UI, plugin system, 3D overlay |
| `egui_extras` | 0.34 | Additional widgets | Syntax highlighting, image viewers, custom widgets |
| `egui_kittest` | 0.34 | UI testing | Unit testing for egui components |
| `egui_async` | 0.34 | Async integration | Tokio/WASM async task binding |
| `egui_hooks` | 0.34 | State management | React-like hooks pattern |

## 🏗️ Advanced App Structure
### Complex Application Patterns
For large applications, use a modular architecture with separate concerns 【turn0search6】:

```rust
use eframe::egui;
use std::sync::{Arc, Mutex};

// Centralized application state
#[derive(Default)]
struct AppState {
    data: Arc<Mutex<Vec<f64>>>,
    settings: Settings,
    active_panel: PanelType,
}

// Modular UI components
trait Panel {
    fn show(&mut self, ctx: &egui::Context, state: &mut AppState);
}

struct MyApp {
    state: AppState,
    panels: Vec<Box<dyn Panel>>,
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Centralized state management
        egui::CentralPanel::default().show(ctx, |ui| {
            // Dispatch to active panel
            for panel in &mut self.panels {
                if panel.is_active(&self.state) {
                    panel.show(ui.ctx(), &mut self.state);
                }
            }
        });
        
        // Background async tasks
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}
```

### Multi-Window Applications
egui supports multiple windows via `ViewportBuilder` for complex desktop applications 【turn0search10】【turn0search12】:

```rust
fn show_secondary_window(ctx: &egui::Context) {
    let viewport = egui::ViewportBuilder::default()
        .with_title("Secondary Window")
        .with_inner_size([400.0, 300.0])
        .with_id(egui::Id::new("secondary"));
    
    ctx.show_viewport(viewport, |ctx, _| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("This is a secondary window!");
            // Secondary window content
        });
    });
}
```

## 🎨 Advanced Styling & Theming
### Dynamic Theme System
Implement a comprehensive theme system with runtime switching:

```rust
use egui::{Style, Visuals, Color32};

struct ThemeManager {
    current_theme: Theme,
    themes: HashMap<String, Theme>,
}

#[derive(Clone, Copy)]
enum Theme {
    Dark,
    Light,
    Custom(Color32),
}

impl ThemeManager {
    fn apply_theme(&self, ctx: &egui::Context) {
        let mut style = Style::default();
        
        match self.current_theme {
            Theme::Dark => {
                style.visuals = Visuals::dark();
                style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(27, 27, 27);
            }
            Theme::Light => {
                style.visuals = Visuals::light();
            }
            Theme::Custom(color) => {
                style.visuals = Visuals::dark();
                style.visuals.widgets.noninteractive.bg_fill = color;
            }
        }
        
        ctx.set_style(style);
    }
}
```

### Custom Font Management
Install and manage custom fonts with emoji support for rich text interfaces:

```rust
use egui::{FontDefinitions, FontFamily, FontData};

fn install_custom_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    
    // Install custom font from file
    fonts.font_data.insert(
        "custom_font".to_owned(),
        FontData::from_static(include_bytes!("../assets/CustomFont.ttf")),
    );
    
    // Register as new font family
    fonts.families.insert(
        FontFamily::Name("Custom".to_owned()),
        vec!["custom_font".to_owned()],
    );
    
    // Add emoji support
    fonts.font_data.insert(
        "emoji_font".to_owned(),
        FontData::from_static(include_bytes!("../assets/NotoEmoji.ttf")),
    );
    fonts.families.entry(FontFamily::Proportional)
        .or_default()
        .push("emoji_font".to_owned());
    
    ctx.set_fonts(fonts);
}
```

## 📊 Data Visualization with egui_plot
### Real-time Plotting
For high-performance real-time data visualization, use `egui_plot` with downsampling algorithms:

```rust
use egui_plot::{Plot, Line, PlotPoints};
use std::sync::{Arc, Mutex};

struct RealTimePlot {
    data: Arc<Mutex<Vec<[f64; 2]>>>,
    max_points: usize,
}

impl RealTimePlot {
    fn new(max_points: usize) -> Self {
        Self {
            data: Arc::new(Mutex::new(Vec::with_capacity(max_points))),
            max_points,
        }
    }
    
    fn add_point(&self, x: f64, y: f64) {
        let mut data = self.data.lock().unwrap();
        data.push([x, y]);
        
        // LTTB downsampling for large datasets
        if data.len() > self.max_points {
            let downsampled = lttd_downsample(&data, self.max_points);
            *data = downsampled;
        }
    }
    
    fn plot(&self, ui: &mut egui::Ui) {
        let data = self.data.lock().unwrap();
        let points: PlotPoints = data.iter().map(|&[x, y]| [x, y]).collect();
        
        Plot::new("real_time_plot")
            .show(ui, |plot_ui| {
                plot_ui.line(Line::new(points).name("Real-time Data"));
            });
    }
}

// Largest Triangle Three Buckets downsampling algorithm
fn lttd_downsample(data: &[[f64; 2]], threshold: usize) -> Vec<[f64; 2]> {
    // Implementation of LTTB algorithm for efficient downsampling
    // Preserves visual characteristics while reducing point count
    // ...
}
```

### Advanced Plot Customization
Create custom plot elements and interactions:

```rust
use egui_plot::{Plot, PlotPoint, PlotTransform};

struct CustomPlot {
    show_legend: bool,
    custom_marker: bool,
}

impl CustomPlot {
    fn show(&mut self, ui: &mut egui::Ui) {
        Plot::new("custom_plot")
            .show_x(true)
            .show_y(true)
            .legend(egui_plot::Legend::default())
            .show(ui, |plot_ui| {
                // Custom plot elements
                plot_ui.line(egui_plot::Line::new(vec![[0.0, 0.0], [1.0, 1.0]])
                    .name("Custom Line")
                    .color(egui::Color32::BLUE));
                
                // Custom markers
                if self.custom_marker {
                    plot_ui.points(egui_plot::Points::new(vec![[0.5, 0.5]])
                        .color(egui::Color32::RED)
                        .radius(5.0));
                }
            });
        
        // Interactive elements
        ui.checkbox(&mut self.show_legend, "Show Legend");
        ui.checkbox(&mut self.custom_marker, "Custom Markers");
    }
}
```

## ⚡ Performance Optimization Techniques
### Immediate-Mode Performance Strategies
Optimize egui applications for high-performance scenarios:

```rust
use egui::util::History;
use std::time::{Duration, Instant};

struct PerformanceOptimizer {
    frame_history: History<f32>,
    last_repaint: Instant,
    needs_repaint: bool,
}

impl PerformanceOptimizer {
    fn new() -> Self {
        Self {
            frame_history: History::new(0..120, 0.5),
            last_repaint: Instant::now(),
            needs_repaint: true,
        }
    }
    
    fn update(&mut self, ctx: &egui::Context) {
        // Adaptive repaint strategy
        let elapsed = self.last_repaint.elapsed();
        
        if self.needs_repaint || elapsed > Duration::from_millis(100) {
            ctx.request_repaint();
            self.last_repaint = Instant::now();
            self.needs_repaint = false;
        }
        
        // Frame time monitoring
        let frame_time = ctx.input(|i| i.unstable_dt);
        self.frame_history.add(frame_time);
        
        // Adaptive quality scaling
        if self.frame_history.average() > 0.033 { // > 30 FPS threshold
            ctx.set_style(egui::Style {
                visuals: egui::Visuals {
                    // Reduce visual complexity
                    ..Default::default()
                },
                ..Default::default()
            });
        }
    }
    
    fn mark_needs_repaint(&mut self) {
        self.needs_repaint = true;
    }
}
```

### Memory Management Patterns
For applications with large datasets or long lifetimes:

```rust
use std::sync::Arc;
use egui::{Context, Id};

struct CachedData {
    data: Arc<Vec<f64>>,
    last_accessed: Instant,
}

struct DataCache {
    cache: HashMap<Id, CachedData>,
    max_size: usize,
}

impl DataCache {
    fn get_or_insert(&mut self, ctx: &Context, id: Id, data: Arc<Vec<f64>>) -> Arc<Vec<f64>> {
        // Check cache first
        if let Some(cached) = self.cache.get(&id) {
            if cached.last_accessed.elapsed() < Duration::from_secs(60) {
                return cached.data.clone();
            }
        }
        
        // Insert new data
        self.cache.insert(id, CachedData {
            data: data.clone(),
            last_accessed: Instant::now(),
        });
        
        // Enforce cache size limit
        if self.cache.len() > self.max_size {
            self.evict_oldest();
        }
        
        data
    }
    
    fn evict_oldest(&mut self) {
        // LRU cache eviction strategy
        if let Some((&oldest_id, _)) = self.cache
            .iter()
            .min_by_key(|(_, cached)| cached.last_accessed)
        {
            self.cache.remove(&oldest_id);
        }
    }
}
```

## 🔗 Advanced Integration Patterns
### Bevy Game Engine Integration
For game development, integrate egui with Bevy using `bevy_egui` 【turn0search5】【turn0search59】:

```rust
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(EguiPlugin)
        .add_system(ui_system)
        .add_system(game_logic_system)
        .run();
}

fn ui_system(
    mut contexts: EguiContexts,
    mut game_state: ResMut<GameState>,
) {
    let ctx = contexts.ctx_mut();
    
    egui::Window::new("Game UI").show(ctx, |ui| {
        ui.label(format!("Player Health: {}", game_state.health));
        ui.label(format!("Score: {}", game_state.score));
        
        if ui.button("Start Game").clicked() {
            game_state.game_started = true;
        }
        
        // Game-specific UI elements
        ui.horizontal(|ui| {
            ui.label("Inventory:");
            for item in &game_state.inventory {
                ui.button(item.name).clicked().then(|| {
                    // Handle item use
                });
            }
        });
    });
}

fn game_logic_system(
    mut game_state: ResMut<GameState>,
    time: Res<Time>,
) {
    if game_state.game_started {
        // Update game state
        game_state.score += time.delta_seconds() as u32;
    }
}
```

### Async Integration with Tokio
For applications requiring async operations, use `egui-async` or manual integration 【turn0search42】【turn0search47】:

```rust
use egui_async::Bind;
use tokio::sync::mpsc;

struct AsyncApp {
    receiver: mpsc::Receiver<Data>,
    loading_state: LoadingState,
    bind: Option<egui_async::Bind<Data>>,
}

impl AsyncApp {
    fn new() -> (Self, mpsc::Sender<Data>) {
        let (tx, rx) = mpsc::channel(100);
        (Self {
            receiver: rx,
            loading_state: LoadingState::Idle,
            bind: None,
        }, tx)
    }
    
    fn spawn_async_task(&mut self, ctx: &egui::Context) {
        self.loading_state = LoadingState::Loading;
        
        // Using egui-async crate
        self.bind = Some(ctx.bind(async {
            fetch_data().await
        }));
    }
    
    fn update(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        // Check for async completions
        if let Some(bind) = &self.bind {
            if let Some(result) = bind.result() {
                self.loading_state = LoadingState::Complete(result);
                ctx.request_repaint();
            }
        }
        
        match &self.loading_state {
            LoadingState::Idle => {
                if ui.button("Load Data").clicked() {
                    self.spawn_async_task(ctx);
                }
            }
            LoadingState::Loading => {
                ui.spinner();
                ui.label("Loading...");
            }
            LoadingState::Complete(data) => {
                ui.label(format!("Data: {:?}", data));
            }
        }
    }
}
```

## 🧪 Testing & Debugging
### UI Testing with egui_kittest
Write unit tests for egui components:

```rust
use egui_kittest:: TestingUI;

#[test]
fn test_button_interaction() {
    let mut ui = TestingUI::new(|ui| {
        let mut clicked = false;
        if ui.button("Click me").clicked() {
            clicked = true;
        }
        clicked
    });
    
    // Simulate click
    ui.click_button("Click me");
    
    // Assert state
    assert!(ui.get_result());
}

#[test]
fn test_layout_responsiveness() {
    let mut ui = TestingUI::new(|ui| {
        ui.horizontal(|ui| {
            ui.button("Button 1");
            ui.button("Button 2");
        });
    });
    
    // Test different screen sizes
    ui.set_width(300.0);
    assert!(ui.is_visible("Button 1"));
    
    ui.set_width(100.0);
    // Check if layout adapts
    assert!(ui.is_visible("Button 1") || ui.is_visible("Button 2"));
}
```

### Debugging Tools
Enable debug features for development:

```rust
use egui::debug;

impl MyApp {
    fn debug_ui(&mut self, ctx: &egui::Context) {
        // Debug overlay
        debug::debug_ui(ctx, |ui| {
            ui.heading("Debug Information");
            ui.label(format!("FPS: {:.1}", ctx.debug_fps()));
            ui.label(format!("Memory: {} KB", ctx.debug_memory_usage()));
            
            // Widget inspector
            if ui.button("Inspect Widgets").clicked() {
                debug::show_widget_inspector(ctx);
            }
            
            // Performance metrics
            if ui.button("Performance Metrics").clicked() {
                debug::show_performance_metrics(ctx);
            }
        });
    }
}
```

## 📋 Production Checklist

### 🚀 **Performance & Optimization**
- [ ] Implement adaptive repaint strategy with `ctx.request_repaint_after()`
- [ ] Use LTTB downsampling for plots with >10,000 data points 【turn0search2】
- [ ] Cache expensive computations and widget states with `egui::Memory`
- [ ] Implement virtual scrolling for long lists using `egui::ScrollArea`
- [ ] Profile with `egui::debug` and Tracy integration 【turn0search5】
- [ ] Optimize binary size with `cargo build --release` and feature flags
- [ ] Implement lazy loading for non-critical UI elements

### 🏗️ **Architecture & Structure**
- [ ] Separate UI logic from business logic using a dispatcher pattern 【turn0search6】
- [ ] Implement modular panel system for complex applications 【turn0search10】
- [ ] Use `Arc<Mutex<T>>` for shared state across threads
- [ ] Implement proper error boundaries and recovery
- [ ] Structure app with clear separation of concerns (MVC/MVVM patterns)
- [ ] Use `egui::Id` for persistent widget state across sessions
- [ ] Implement undo/redo functionality for user actions

### 🎨 **Styling & User Experience**
- [ ] Create custom theme system with dark/light mode support 【turn0search11】
- [ ] Install custom fonts with emoji support for internationalization 【turn0search25】【turn0search27】
- [ ] Implement responsive layouts that adapt to window resizing 【turn0search12】
- [ ] Add keyboard navigation and shortcuts
- [ ] Provide visual feedback for all user interactions
- [ ] Implement high-contrast mode for accessibility 【turn0search15】
- [ ] Use consistent spacing and alignment with `egui::style::Spacing`

### ♿ **Accessibility & Inclusivity**
- [ ] Enable `AccessKit` for native screen reader support 【turn0search15】【turn0search16】
- [ ] Provide keyboard alternatives for all mouse interactions
- [ ] Implement ARIA labels and roles via `egui::WidgetInfo`
- [ ] Support font scaling for visually impaired users 【turn0search2】
- [ ] Implement colorblind-friendly palettes 【turn0search2】
- [ ] Test with screen readers on target platforms
- [ ] Provide text alternatives for non-text content

### 🔧 **Development & Deployment**
- [ ] Set up CI/CD pipeline for both native and web builds
- [ ] Implement automated UI testing with `egui_kittest` 【turn0search3】
- [ ] Create comprehensive documentation and examples
- [ ] Implement proper logging and error reporting
- [ ] Configure proper window icons and metadata 【turn0search11】
- [ ] Implement crash recovery and state persistence 【turn0search19】
- [ ] Test on all target platforms (Windows, macOS, Linux, Web)

### 🔄 **Advanced Integrations**
- [ ] Integrate with game engines using `bevy_egui` 【turn0search5】【turn0search59】
- [ ] Implement async task integration with Tokio 【turn0search42】【turn0search47】
- [ ] Add real-time data visualization with `egui_plot` 【turn0search1】【turn0search3】
- [ ] Implement custom rendering backends (wgpu/glow) 【turn0search20】【turn0search23】
- [ ] Add support for multi-window applications 【turn0search10】【turn0search13】
- [ ] Implement plugin system for extensibility
- [ ] Add support for platform-specific features (file dialogs, system trays)

## 📚 Additional Resources

<details>
<summary>🔧 **Development Tools & Crates**</summary>

### **Core Crates**
- **`egui`**: Core GUI library 【turn0search30】
- **`eframe`**: Application framework for web/native 【turn0search16】
- **`egui_plot`**: 2D plotting library 【turn0search1】【turn0search3】
- **`egui_extras`**: Additional widgets and utilities
- **`egui_kittest`**: UI testing framework 【turn0search3】

### **Integration Crates**
- **`bevy_egui`**: Bevy game engine integration 【turn0search5】【turn0search59】
- **`egui_async`**: Async task integration 【turn0search42】
- **`egui_hooks`**: React-like hooks pattern 【turn0search32】
- **`egui-router`**: Routing for multi-view applications
- **`egui-dnd`**: Drag-and-drop functionality

### **Development Tools**
- **`eframe_template`**: Project template 【turn0search8】
- **`egui-wgpu`**: WebGPU rendering backend 【turn0search20】
- **`egui_glow`**: OpenGL rendering backend 【turn0search23】
- **`egui-winit`**: Windowing integration
- **`accesskit`**: Accessibility framework 【turn0search15】
</details>

<details>
<summary>📖 **Learning Resources**</summary>

### **Official Resources**
- [egui GitHub Repository](https://github.com/emilk/egui) 【turn0search30】
- [egui Documentation](https://docs.rs/egui) 【turn0search8】
- [egui Web Demo](https://www.egui.rs/#demo) 【turn0search8】
- [egui Changelog](https://github.com/emilk/egui/blob/master/CHANGELOG.md) 【turn0search25】

### **Tutorials & Guides**
- [Getting Started with egui](https://whoisryosuke.com/blog/2023/getting-started-with-egui-in-rust) 【turn0search10】
- [Building Cross-Platform GUI Apps with Rust and egui](https://blog.logrocket.com/building-cross-platform-gui-apps-rust-using-egui) 【turn0search14】
- [Shipping Realtime Desktop Software with Rust, Bevy, and egui](https://nominal.io/blog/nominal-connect-shipping-realtime-desktop-software-with-rust-bevy-and-egui) 【turn0search5】

### **Community**
- [egui Discussions](https://github.com/emilk/egui/discussions)
- [Rust GUI Reddit Community](https://www.reddit.com/r/rust/)
- [egui Discord Server](https://discord.gg/egui)
</details>

<details>
<summary>🔄 **Migration Guides**</summary>

### **Migrating to 0.34.x**
```rust
// Old 0.33 code
let galley = ui.fonts(|f| f.layout_job(text_job));

// New 0.34 code
let galley = ctx.fonts(|f| f.layout_job(text_job));

// Old viewport creation
let viewport = egui::ViewportBuilder::default();

// New viewport with ID
let viewport = egui::ViewportBuilder::default()
    .with_id(egui::Id::new("secondary_window"));
```

### **Backend Migration**
```rust
// For lightweight applications
let options = eframe::NativeOptions {
    renderer: eframe::Renderer::Glow,
    ..Default::default()
};

// For advanced graphics (default)
let options = eframe::NativeOptions {
    renderer: eframe::Renderer::Wgpu,
    ..Default::default()
};
```

### **Plot Migration**
```rust
// Old egui plot code
use egui::plot::{Plot, Line, Value};

Plot::new("my_plot").show(ui, |plot_ui| {
    plot_ui.line(Line::new(Value::new(0.0, 0.0)));
});

// New egui_plot 0.35 code
use egui_plot::{Plot, Line, PlotPoints};

Plot::new("my_plot").show(ui, |plot_ui| {
    plot_ui.line(Line::new(PlotPoints::from(vec![[0.0, 0.0]])));
});
```
</details>

## 🎯 Best Practices Summary

### **Performance First**
1. **Profile before optimizing** - Use built-in debug tools
2. **Minimize allocations** - Reuse buffers and containers
3. **Implement caching** - Cache expensive computations
4. **Use adaptive repaints** - Only repaint when necessary
5. **Optimize rendering** - Clip and cull invisible content

### **Clean Architecture**
1. **Separate concerns** - UI logic from business logic
2. **Modular components** - Reusable and testable widgets
3. **State management** - Centralized with clear ownership
4. **Error handling** - Graceful degradation and recovery
5. **Documentation** - Inline and API documentation

### **User Experience**
1. **Responsive design** - Adapt to window sizes
2. **Accessibility** - Support assistive technologies
3. **Consistent styling** - Follow design system
4. **Feedback** - Visual responses to user actions
5. **Internationalization** - Support multiple languages

### **Development Workflow**
1. **Testing** - Unit tests with `egui_kittest`
2. **CI/CD** - Automated builds and tests
3. **Version pinning** - Stable dependency versions
4. **Migration planning** - Track breaking changes
5. **Community engagement** - Contribute back upstream

## 🔮 Future Trends & Considerations

### **WebAssembly Improvements**
- Better performance and smaller binaries
- Improved browser integration
- WebGPU backend maturation 【turn0search20】【turn0search22】

### **Ecosystem Growth**
- More specialized widgets and components
- Better integration with game engines
- Improved tooling and debugging 【turn0search5】【turn0search59】

### **Performance Optimizations**
- GPU-accelerated rendering
- Multi-threaded UI updates
- Predictive frame scheduling

### **Accessibility Enhancements**
- Full platform support for AccessKit 【turn0search15】【turn0search16】
- Better screen reader compatibility
- Voice control integration

---

*This guru skill document is based on egui version 0.34.3 and related ecosystem crates as of mid-2026. For the most current information, always refer to the [official documentation](https://docs.rs/egui) and [changelog](https://github.com/emilk/egui/blob/master/CHANGELOG.md).*
```