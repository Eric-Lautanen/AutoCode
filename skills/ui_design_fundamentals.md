---
name: ui-design-fundamentals
description: Use when building any user interface - web, desktop, or mobile - and design decisions need to be made about layout, spacing, typography, color, or visual hierarchy. Load when asked to make something look good, improve a UI, design a component, or when starting a frontend task from scratch.
---

# UI Design Fundamentals

## Overview

Good UI design isn't about artistic talent — it's about applying a small set of principles consistently. Visual hierarchy, spacing, typography, and color are the building blocks. When these are systematic (using scales and tokens, not arbitrary values), even developers who "aren't designers" can produce clean, professional interfaces. This skill covers the core visual design principles that apply to any UI platform.

For component-level design decisions, see `component_design.md`. For CSS implementation, see `css_layout.md` and `css_styling.md`.

## Visual Hierarchy

The most important principle: guide the user's eye to what matters most.

**Tools for creating hierarchy (in order of impact):**
1. **Size** — larger = more important
2. **Weight** — bold = more important
3. **Color/contrast** — high contrast = more important
4. **Position** — top-left (in LTR) = seen first
5. **Whitespace** — isolated elements draw attention

**Apply hierarchy to every page:**
- What's the one thing the user should see first? Make it the biggest, highest-contrast element.
- What's secondary? Smaller, lighter weight, or muted color.
- What's tertiary? Even smaller, lowest contrast acceptable for readability.

**Anti-pattern**: Everything is the same size and weight. The page feels flat and the user doesn't know where to look.

## Spacing System

Never use arbitrary values. Use a consistent scale based on a single unit:

**4px base scale (most common):**
```
4px   — micro (icon gap, inline spacing)
8px   — xs (tight element spacing)
12px  — sm (related elements)
16px  — md (default component padding)
24px  — lg (section separation)
32px  — xl (major section gaps)
48px  — 2xl (page-level separation)
64px  — 3xl (hero-level spacing)
```

**Rules:**
- Use multiples of your base unit (4px or 8px) — never `13px` or `17px`
- Related elements: tighter spacing (8-12px)
- Unrelated elements: wider spacing (24-48px)
- More whitespace = more separation = perceived as less related
- Consistent spacing is more important than the exact values you choose

## Typography

### Font Selection

- **2 fonts maximum**: one for headings, one for body. Often one font with multiple weights is enough.
- **System font stacks** are fast and familiar: `-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif`
- **Avoid decorative fonts** for body text — readability trumps personality

### Type Scale

Use a modular scale for font sizes. Common pattern:

```
12px — captions, labels
14px — small body, secondary text
16px — body text (base)
20px — large body, lead paragraph
24px — h3 / section headings
32px — h2 / page headings
48px — h1 / hero headings
```

### Line Height and Letter Spacing

- Body text: `line-height: 1.5` to `1.6` (relative to font-size)
- Headings: `line-height: 1.1` to `1.3` (tighter because larger text has built-in visual space)
- Letter spacing: `0` for body, `-0.02em` to `-0.04em` for large headings (tighter tracking looks better at large sizes)

## Color

### Color System Structure

```
Primitive palette:  blue-50 through blue-900, gray-50 through gray-900, etc.
Semantic layer:     color.text.primary, color.text.secondary, color.bg.surface, color.border.default
Component tokens:   button.primary.bg, input.border.focus
```

- **Primitive palette**: The raw color values. Never use these directly in components.
- **Semantic tokens**: Named by purpose. `color.text.error` not `color.red.500`. This enables theming.
- **Component tokens**: Override semantic tokens for specific components when needed.

### Semantic Colors

| Purpose | Usage |
|---------|-------|
| Primary | Brand color, primary buttons, links, active states |
| Secondary | Secondary actions, subtle emphasis |
| Neutral | Text, backgrounds, borders, dividers |
| Error/Danger | Errors, destructive actions, validation failures |
| Success | Confirmations, completed states |
| Warning | Caution, pending states |

### Contrast and Accessibility

- **Normal text**: minimum 4.5:1 contrast ratio against background (WCAG AA)
- **Large text** (18px+ bold or 24px+): minimum 3:1
- **UI components and graphical objects**: minimum 3:1
- Test with a contrast checker tool. What looks readable to you may not pass.

## Layout Principles

- **Alignment**: Everything aligns to something. Left-align text, right-align numbers, center only for hero content.
- **Grid**: Use a grid (12-column is standard for web). It creates consistent alignment across the page.
- **Whitespace as a design tool**: Don't fill every pixel. Whitespace separates, groups, and gives the eye rest.
- **Consistent margins**: Same margin on all sides of a group, not different values "because it looked better."

## Dark Mode

Design for both from the start. It's much harder to retrofit.

- **Use semantic color tokens** — swap the token values, not the component code
- **Don't just invert**: dark mode isn't white-on-black. It's dark surfaces with adjusted colors.
- **Reduce saturation** in dark mode: bright saturated colors on dark backgrounds cause eye strain and halation
- **Shadows don't work** on dark backgrounds — use subtle borders or different surface levels instead
- **Test both modes** for contrast — a color that passes in light mode may fail in dark

## Common Mistakes

| Mistake | Fix |
|---------|-----|
| Too many colors | Stick to primary + neutral + 2-3 semantic colors |
| Inconsistent spacing | Use a spacing scale, no arbitrary values |
| Low contrast text | Check contrast ratio, use a checker tool |
| Center-aligned body text | Left-align body text, center only short headings or heroes |
| No visual hierarchy | Make the primary action obviously primary (size, weight, color) |
| Different button styles for same action type | Reuse the same component with same variants |
| Ignoring dark mode | Design with semantic tokens from day one |

## Windows-Specific Notes

### Windows UI Frameworks
When designing for Windows applications:
- **WinUI 3**: Modern Windows UI framework. Uses Fluent Design System.
- **WPF (Windows Presentation Foundation)**: Older but still widely used. XAML-based.
- **WinForms**: Legacy, but simple for quick tools.

### Windows High DPI Handling
Windows handles DPI scaling at the OS level:
- **Per-monitor DPI**: Apps should handle `WM_DPICHANGED` message
- **Scaling values**: 100%, 125%, 150%, 175%, 200%, 225%, 250%, 300%, 400%, 500%
- **Design at 100% (96 DPI)**: Windows scales up. Test at multiple DPI settings

```css
/* CSS for Windows high DPI */
@media (min-resolution: 120dpi) {
  /* Adjustments for 125% scaling */
}

@media (min-resolution: 144dpi) {
  /* Adjustments for 150% scaling */
}
```

### Windows Font Rendering
Windows ClearType renders fonts differently than macOS:
- **Segoe UI**: Windows system font. Use for native Windows apps
- **Arial/Helvetica**: Common fallbacks, but Segoe UI is preferred on Windows
- **Font smoothing**: Windows uses subpixel rendering (ClearType). Test text legibility

### Windows Dark Mode
Windows 10/11 has system-wide dark mode:
```css
/* Detect Windows dark mode preference */
@media (prefers-color-scheme: dark) {
  :root {
    --bg-primary: #1e1e1e;
    --text-primary: #ffffff;
  }
}
```

## Checklist

- [ ] Visual hierarchy is clear — the most important element is visually dominant
- [ ] Spacing uses a consistent scale (no arbitrary pixel values)
- [ ] Typography uses a type scale with consistent line heights
- [ ] Color uses semantic tokens, not raw palette values
- [ ] Contrast ratios meet WCAG AA minimums
- [ ] Layout uses a grid system with consistent alignment
- [ ] Dark mode is designed with semantic tokens, not just color inversion
- [ ] No more than 2 font families
