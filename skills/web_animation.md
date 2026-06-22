---
name: web-animation
description: Use when implementing animations on the web - CSS transitions, CSS animations, JavaScript-driven animation, scroll-triggered effects, and performance-safe motion. Load when a task involves animating UI elements, implementing motion design, or when animations are janky or causing performance problems.
---

# Web Animation

## Overview

Animation on the web serves two purposes: **feedback** (confirming an action happened) and **orientation** (showing where things came from and where they went). Good animation is purposeful, brief, and smooth. Bad animation is decorative, slow, and janky. This skill covers the animation techniques that work reliably, the performance rules that keep them smooth, and the accessibility requirement to respect reduced motion preferences.

For CSS transitions and transforms, see `css_styling.md`. For DOM manipulation, see `javascript_dom.md`.

## CSS Transitions

The simplest animation: smoothly interpolate between two states.

```css
.element {
  opacity: 1;
  transform: translateY(0);
  transition: opacity 0.3s ease, transform 0.3s ease;
}

.element.hidden {
  opacity: 0;
  transform: translateY(10px);
}
```

### Which Properties to Transition

| Safe (GPU-composited) | Unsafe (causes layout) |
|----------------------|----------------------|
| `transform` | `width`, `height` |
| `opacity` | `top`, `left`, `right`, `bottom` |
| `filter` | `margin`, `padding` |
| `clip-path` | `font-size` |
| `background-color` | `border-width` |

**Rule**: Only transition `transform` and `opacity` for 60fps. Everything else causes layout reflow and jank.

### Easing Functions

```css
/* Common easing curves */
transition-timing-function: ease;              /* Default: slow start, fast middle, slow end */
transition-timing-function: ease-out;          /* Fast start, slow end — good for entrances */
transition-timing-function: ease-in;           /* Slow start, fast end — good for exits */
transition-timing-function: ease-in-out;       /* Symmetric — good for state changes */
transition-timing-function: linear;            /* Constant speed — good for spinners, progress */

/* Custom cubic-bezier */
transition-timing-function: cubic-bezier(0.4, 0, 0.2, 1);  /* Material standard */
transition-timing-function: cubic-bezier(0, 0, 0.2, 1);     /* Material deceleration */
transition-timing-function: cubic-bezier(0.4, 0, 1, 1);     /* Material acceleration */
```

### Duration Guidelines

| Context | Duration |
|---------|----------|
| Micro-interactions (hover, press) | 100-150ms |
| Small transitions (tooltip, dropdown) | 150-200ms |
| Medium transitions (modal, panel) | 200-300ms |
| Large transitions (page change, layout shift) | 300-500ms |
| Complex/choreographed animations | 500ms-1s |

**Rule**: Most animations should be 150-300ms. Anything over 500ms feels slow.

## CSS Animations

For multi-step or looping animations:

```css
@keyframes slideIn {
  from {
    opacity: 0;
    transform: translateY(20px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.element {
  animation: slideIn 0.3s ease-out forwards;
}

/* Shorthand: name duration timing-function delay iteration-count direction fill-mode */
animation: slideIn 0.3s ease-out 0.1s 1 normal forwards;
```

### Animation Properties

```css
.element {
  animation-name: slideIn;
  animation-duration: 0.3s;
  animation-timing-function: ease-out;
  animation-delay: 0.1s;          /* Wait before starting */
  animation-iteration-count: 1;    /* or: infinite */
  animation-direction: normal;     /* normal, reverse, alternate, alternate-reverse */
  animation-fill-mode: forwards;   /* Keep end state after animation */
  animation-play-state: running;   /* running or paused */
}
```

### fill-mode

| Value | What it does |
|-------|-------------|
| `none` | Returns to original state after animation |
| `forwards` | Keeps the final keyframe state |
| `backwards` | Applies the first keyframe during delay |
| `both` | Both backwards and forwards |

**Common need**: `forwards` — keep the element in its animated state after the animation ends.

## JavaScript Animation

### When JS Is Necessary

- **Complex sequences**: Multiple elements with staggered timing
- **Physics-based animation**: Spring dynamics, momentum
- **Scroll-linked animation**: Parallax, progress indicators
- **Interactive animation**: Drag, gesture-based movement
- **Dynamic values**: Animation targets computed at runtime

### requestAnimationFrame

```javascript
function animate({ duration, easing, draw }) {
  const start = performance.now();

  requestAnimationFrame(function tick(now) {
    const elapsed = now - start;
    const progress = Math.min(elapsed / duration, 1);
    const easedProgress = easing(progress);

    draw(easedProgress);

    if (progress < 1) {
      requestAnimationFrame(tick);
    }
  });
}

// Usage
animate({
  duration: 300,
  easing: (t) => t * (2 - t),  // ease-out quad
  draw: (progress) => {
    element.style.transform = `translateY(${(1 - progress) * 20}px)`;
    element.style.opacity = progress;
  }
});
```

### Web Animations API

Native browser API — more powerful than CSS animations, no library needed:

```javascript
const animation = element.animate(
  [
    { opacity: 0, transform: 'translateY(20px)' },
    { opacity: 1, transform: 'translateY(0)' }
  ],
  {
    duration: 300,
    easing: 'ease-out',
    fill: 'forwards'
  }
);

// Control playback
animation.pause();
animation.play();
animation.reverse();
animation.cancel();

// Respond to completion
animation.finished.then(() => console.log('done'));
```

## Scroll-Triggered Animation

### IntersectionObserver Pattern

```javascript
const observer = new IntersectionObserver(
  (entries) => {
    entries.forEach((entry) => {
      if (entry.isIntersecting) {
        entry.target.classList.add('animate-in');
        observer.unobserve(entry.target);  // Animate once
      }
    });
  },
  { threshold: 0.1 }  // Trigger when 10% visible
);

document.querySelectorAll('.reveal').forEach((el) => observer.observe(el));
```

```css
.reveal {
  opacity: 0;
  transform: translateY(20px);
  transition: opacity 0.6s ease, transform 0.6s ease;
}

.reveal.animate-in {
  opacity: 1;
  transform: translateY(0);
}
```

**Why not scroll event listeners**: Scroll events fire on every pixel — expensive and janky. IntersectionObserver is asynchronous and doesn't block the main thread.

## will-change

```css
/* Tell the browser this property will change — promote to its own layer */
.card:hover {
  will-change: transform;
}

/* REMOVE will-change after animation — it consumes memory */
.animated {
  will-change: transform;
  transition: transform 0.3s ease;
}

/* After transition ends, remove will-change */
```

**Rules**:
- Use sparingly — each `will-change` creates a new compositing layer (memory cost)
- Apply before the animation starts, remove after
- Don't apply to too many elements (memory pressure)
- If you're animating with JS, add `will-change` before starting, remove on `animationend`

## Reduced Motion

**Always respect the user's preference for reduced motion:**

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
    scroll-behavior: auto !important;
  }
}
```

This doesn't remove the state changes — it just makes them instant. Elements still appear/disappear, they just don't animate.

**In JavaScript**:

```javascript
const prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

if (prefersReducedMotion) {
  element.style.opacity = 1;  // Instant, no animation
} else {
  element.animate([...], { duration: 300 });
}
```

## Common Animation Patterns

### Fade In

```css
@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}
```

### Slide Up + Fade In (Most Common Entrance)

```css
@keyframes slideUp {
  from { opacity: 0; transform: translateY(20px); }
  to { opacity: 1; transform: translateY(0); }
}
```

### Staggered List

```css
.list-item {
  opacity: 0;
  animation: slideUp 0.3s ease-out forwards;
}

.list-item:nth-child(1) { animation-delay: 0ms; }
.list-item:nth-child(2) { animation-delay: 50ms; }
.list-item:nth-child(3) { animation-delay: 100ms; }
```

### Exit Animation

```css
.element {
  transition: opacity 0.2s ease, transform 0.2s ease;
}

.element.exiting {
  opacity: 0;
  transform: scale(0.95);
}

/* In JS: add .exiting, wait for transitionend, then remove element */
element.classList.add('exiting');
element.addEventListener('transitionend', () => element.remove());
```

## Windows-Specific Notes

### Windows Browser Considerations
Windows users primarily use:
- **Edge**: Chromium-based, supports all modern web APIs
- **Chrome**: Full support for modern animations
- **Firefox**: Good support, occasional differences in composite timing

All modern Windows browsers support:
- `requestAnimationFrame`
- CSS Animations and Transitions
- Web Animations API
- IntersectionObserver

### Windows High DPI and Animation

Windows handles DPI scaling which can affect animations:
- **Subpixel rendering**: At 125%, 150%, 175% scaling, `transform: translate()` may snap to different subpixel positions
- **Test at multiple DPI settings**: Animations that look smooth at 100% may jitter at 150%
- **Use `transform` over `left`/`top`**: More resilient to DPI scaling issues

### Windows Reduced Motion
Windows has a system-wide setting for reduced motion:
```css
@media (prefers-reduced-motion: reduce) {
  /* Respects Windows accessibility settings */
}
```

### Performance on Windows
- **Antivirus scanning**: Real-time protection can cause frame drops during heavy animation
- **GPU drivers**: Outdated Intel/AMD/NVIDIA drivers are a common cause of animation jank on Windows
- **Power modes**: Windows "Battery Saver" or "Best Power Efficiency" modes may throttle animations

## Checklist

- [ ] Only animating `transform` and `opacity` (GPU-composited properties)
- [ ] Durations are 150-300ms for most transitions
- [ ] Easing uses ease-out for entrances, ease-in for exits
- [ ] `prefers-reduced-motion` respected — animations disabled for users who prefer it
- [ ] `will-change` used sparingly and removed after animation
- [ ] Scroll-triggered animations use IntersectionObserver, not scroll events
- [ ] Exit animations wait for `transitionend`/`animationend` before removing elements
- [ ] No animation that runs longer than 500ms without a clear purpose
