---
name: browser-performance
description: Use when a web page or app feels slow, has poor Core Web Vitals scores, or needs optimization for load time, rendering performance, or runtime smoothness. Load when asked to improve page speed, fix jank, reduce bundle size, or optimize any frontend performance metric.
---

# Browser Performance

## Overview

Browser performance has three dimensions: **loading** (how fast content appears), **interactivity** (how fast the page responds to input), and **visual stability** (how much the layout shifts while loading). These are measured by Core Web Vitals (LCP, INP, CLS). This skill covers the most impactful optimizations for each dimension, with practical patterns you can apply immediately.

For image optimization (often the biggest win), see `responsive_images_and_media.md`. For animation performance, see `web_animation.md`.

## Core Web Vitals

| Metric | What it measures | Good | Poor |
|---------|-----------------|------|------|
| **LCP** (Largest Contentful Paint) | Load: when the largest visible content renders | <2.5s | >4s |
| **INP** (Interaction to Next Paint) | Interactivity: how fast the page responds to clicks/keys | <200ms | >500ms |
| **CLS** (Cumulative Layout Shift) | Stability: how much layout shifts during load | <0.1 | >0.25 |

**Priority**: Fix LCP first (users see content), then CLS (content doesn't jump), then INP (interactions feel fast).

## Critical Rendering Path

The browser must complete these steps before anything appears on screen:

```
HTML → Parse → DOM
CSS → Parse → CSSOM → Render Tree → Layout → Paint → Composite
JS → Execute → (may modify DOM/CSSOM, restart the path)
```

### What Blocks Rendering

| Resource | Blocks? | Fix |
|----------|---------|-----|
| CSS in `<head>` | **Yes** (browser needs CSSOM before paint) | Inline critical CSS, defer the rest |
| `<script>` in `<head>` | **Yes** (blocks DOM parsing) | `defer` or `async` |
| Render-blocking fonts | **Yes** (text invisible until font loads) | `font-display: swap`, preload critical fonts |
| Synchronous XHR | **Yes** | Never use synchronous XHR |

### Script Loading Strategies

```html
<!-- Blocks parsing, executes immediately -->
<script src="app.js"></script>

<!-- Downloads during parsing, executes after DOM ready (preserves order) -->
<script src="app.js" defer></script>

<!-- Downloads during parsing, executes ASAP when downloaded (no order guarantee) -->
<script src="analytics.js" async></script>

<!-- Inline critical JS, defer everything else -->
<script>
  // Minimal critical JS (e.g., theme detection, above-fold interactivity)
</script>
<script src="app.js" defer></script>
```

**Default**: Use `defer` for all scripts. Use `async` only for independent scripts (analytics, ads).

## Bundle Size

### Code Splitting

Load only the code needed for the current page:

```javascript
// Route-based splitting (React Router example)
const HomePage = React.lazy(() => import('./HomePage'));
const AboutPage = React.lazy(() => import('./AboutPage'));

// Component-based splitting (heavy component)
const HeavyChart = React.lazy(() => import('./HeavyChart'));
```

### Tree Shaking

Import only what you use:

```javascript
// Bad: imports entire lodash
import _ from 'lodash';

// Good: imports only the function
import debounce from 'lodash/debounce';

// Best: use lighter alternatives
import { debounce } from 'es-toolkit';  // Modern, smaller alternative
```

### Bundle Analysis

```bash
# Analyze bundle size
npx webpack-bundle-analyzer dist/stats.json
npx source-map-explorer dist/main.js
```

**Targets**:
- First-party JS: <100KB compressed for initial load
- Total JS: <300KB compressed for initial route
- Individual lazy-loaded chunks: <50KB compressed

## Network Optimization

### Resource Hints

```html
<!-- Preconnect: establish connection early (DNS + TCP + TLS) -->
<link rel="preconnect" href="https://api.example.com">

<!-- Prefetch: fetch a resource that will be needed soon -->
<link rel="prefetch" href="/about-page.js">

<!-- Preload: fetch a resource needed for current navigation, high priority -->
<link rel="preload" href="/critical-font.woff2" as="font" type="font/woff2" crossorigin>

<!-- Modulepreload: preload an ES module and its dependencies -->
<link rel="modulepreload" href="/app.js">
```

### Caching Headers

```
# Immutable assets (hashed filenames): cache forever
Cache-Control: public, max-age=31536000, immutable

# HTML: short cache, must revalidate
Cache-Control: no-cache

# API responses: short cache
Cache-Control: private, max-age=60
```

### CDN

Serve static assets from a CDN (Cloudflare, Fastly, CloudFront):
- Reduces latency (served from edge, closer to user)
- Offloads origin server
- Enables HTTP/2 and HTTP/3 automatically

## Runtime Performance

### Avoid Layout Thrashing

Reading layout properties forces the browser to calculate layout. Mixing reads and writes causes thrashing:

```javascript
// Bad: read-write-read-write
elements.forEach(el => {
  const h = el.offsetHeight;     // Read → forces layout
  el.style.height = h * 2 + 'px'; // Write → invalidates layout
});

// Good: batch reads, then batch writes
const heights = elements.map(el => el.offsetHeight);
elements.forEach((el, i) => {
  el.style.height = heights[i] * 2 + 'px';
});
```

### Debounce and Throttle Event Handlers

```javascript
// Debounce: wait until events stop, then fire once
const handleResize = debounce(() => recalculate(), 150);
window.addEventListener('resize', handleResize);

// Throttle: fire at most once per interval
const handleScroll = throttle(() => updatePosition(), 16);
window.addEventListener('scroll', handleScroll);
```

### Virtual Lists for Long Lists

Rendering 10,000 DOM nodes is slow. Render only the visible ones:

```jsx
// React: react-window or react-virtuoso
import { FixedSizeList } from 'react-window';

<FixedSizeList height={600} itemCount={10000} itemSize={50}>
  {({ index, style }) => (
    <div style={style}>{items[index].name}</div>
  )}
</FixedSizeList>
```

### Web Workers for Heavy Computation

Move CPU-intensive work off the main thread:

```javascript
// main.js
const worker = new Worker('processor.js');
worker.postMessage({ data: largeDataset });
worker.onmessage = (e) => {
  updateUI(e.data.result);
};

// processor.js
self.onmessage = (e) => {
  const result = heavyComputation(e.data.data);
  self.postMessage({ result });
};
```

## Images (The Biggest Win)

Images are usually the largest assets and the LCP element. See `responsive_images_and_media.md` for full details.

**Quick wins**:
1. Serve WebP/AVIF with JPEG fallback
2. Set `width` and `height` to prevent CLS
3. Lazy-load below-the-fold images
4. Preload the LCP image: `<link rel="preload" as="image" href="hero.webp">`
5. Compress: serve at display size, not source size

## Measuring Performance

### Lab Data (Controlled Environment)

| Tool | What it measures |
|------|-----------------|
| **Lighthouse** | All Core Web Vitals, opportunities, diagnostics |
| **Chrome DevTools Performance** | Runtime: layout, paint, scripting breakdown |
| **WebPageTest** | Detailed waterfall, filmstrip, connection info |

### Field Data (Real Users)

| Tool | What it measures |
|------|-----------------|
| **Chrome UX Report** (CrUX) | Real-user Core Web Vitals by origin |
| **web-vitals library** | Measure CLS, LCP, INP in your own analytics |
| **RUM (Real User Monitoring)** | New Relic, Datadog, SpeedCurve — full performance telemetry |

### Web Vitals in Code

```javascript
import { onLCP, onINP, onCLS } from 'web-vitals';

onLCP((metric) => {
  analytics.track('LCP', { value: metric.value, element: metric.element });
});
onINP((metric) => {
  analytics.track('INP', { value: metric.value });
});
onCLS((metric) => {
  analytics.track('CLS', { value: metric.value });
});
```

## Windows-Specific Performance Notes

### Antivirus Scanning Impact
On Windows, antivirus software can significantly impact file I/O and build performance:

- **Exclude development directories** from real-time scanning (e.g., `node_modules`, `.git`, build output)
- **Add exceptions** for your IDE and build tools
- **Common exclusions**:
  - Project directories: `C:\Users\<name>\projects\`
  - Package caches: `C:\Users\<name>\AppData\Local\npm-cache\`
  - Build output: `dist\`, `build\`, `.next\`

### Windows Defender Exclusions (PowerShell)
```powershell
# Add exclusion for a project directory
Add-MpPreference -ExclusionPath "C:\Users\$env:USERNAME\projects"

# Add exclusion for Node.js
Add-MpPreference -ExclusionProcess "node.exe"
```

### Path Length Limitations
Windows has a 260-character path limit (MAX_PATH) by default:
- **Enable long path support** in Windows 10/11: Group Policy or Registry edit
- **Use `\\?\` prefix** for absolute paths in scripts
- **Prefer shorter paths** for project directories (e.g., `C:\dev\` instead of `C:\Users\VeryLongUsername\Documents\Projects\`)

### NTFS vs. Other Filesystems
- **NTFS**: Journaling adds slight overhead but ensures data integrity
- **ReFS**: Better for large files, but limited Windows support
- **WSL2 ext4**: Faster for Linux-native toolchains, but file access across WSL/Windows boundary is slow

## Checklist

- [ ] LCP < 2.5s (preload LCP image, inline critical CSS, defer scripts)
- [ ] CLS < 0.1 (set width/height on images, avoid late-injecting content)
- [ ] INP < 200ms (debounce handlers, use web workers for heavy work)
- [ ] Scripts loaded with `defer` (not blocking)
- [ ] Critical CSS inlined, non-critical CSS loaded asynchronously
- [ ] Images in WebP/AV累积 with fallbacks, lazy-loaded below fold
- [ ] Bundle analyzed and under size targets
- [ ] Caching headers set correctly (immutable for hashed assets)
- [ ] Performance measured with both lab and field data
- [ ] Windows: Development directories excluded from antivirus scanning
- [ ] Windows: Path lengths kept under 260 characters or long paths enabled
