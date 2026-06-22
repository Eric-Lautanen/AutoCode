---
name: responsive-images-and-media
description: Use when optimizing images, implementing responsive images, handling video, or managing media assets on the web. Load when a task involves image performance, different screen densities, art direction for different viewports, or slow page load caused by unoptimized media.
---

# Responsive Images and Media

## Overview

Images are typically the largest assets on a web page and the biggest factor in page load performance. Serving the right image at the right size, format, and resolution can cut page weight by 50% or more. This skill covers image formats, responsive image techniques, video handling, and SVG optimization — everything you need to serve media efficiently.

For browser performance optimization, see `browser_performance.md`. For HTML structure, see `html_structure.md`.

## Image Formats

### When to Use Each Format

| Format | Best for | Transparency | Animation | Compression |
|--------|----------|-------------|-----------|-------------|
| **JPEG** | Photos, complex images | No | No | Lossy |
| **PNG** | Screenshots, graphics with text, sharp edges | Yes | No | Lossless |
| **WebP** | Modern replacement for JPEG and PNG | Yes | Yes | Both (lossy/lossless) |
| **AVIF** | Best compression, modern browsers | Yes | Yes | Both |
| **SVG** | Icons, logos, illustrations, diagrams | Yes | Yes (SMIL/CSS) | Vector (infinite resolution) |
| **GIF** | Short animations (prefer video or WebP) | Yes | Yes | Limited palette |

### Decision Tree

```
Is it a photo?
  → Yes: Use WebP (fallback: JPEG)
  → No: Is it an icon/logo/illustration?
    → Yes: Use SVG
    → No: Is it a screenshot or graphic with text?
      → Yes: Use WebP (fallback: PNG)
      → No: Use WebP (fallback: PNG or JPEG depending on content)
```

### Format Support and Fallbacks

```html
<picture>
  <source srcset="photo.avif" type="image/avif">
  <source srcset="photo.webp" type="image/webp">
  <img src="photo.jpg" alt="Description" width="800" height="600">
</picture>
```

Browser picks the first format it supports. AVIF → WebP → JPEG.

## srcset and sizes

### Resolution Switching (Same Image, Different Sizes)

Serve the right resolution for the device:

```html
<img src="photo-800.jpg"
     srcset="photo-400.jpg 400w, photo-800.jpg 800w, photo-1200.jpg 1200w, photo-1600.jpg 1600w"
     sizes="(max-width: 600px) 100vw, (max-width: 1200px) 50vw, 800px"
     alt="Description">
```

**How it works**:
1. Browser evaluates `sizes` to determine the display width (e.g., "100vw" on mobile = 400px)
2. Browser picks the best source from `srcset` based on device pixel ratio and display width
3. On a 2x mobile (800px physical pixels), it picks `photo-800.jpg`

### Density Descriptors (For Icons and Small Images)

```html
<img src="icon.png"
     srcset="icon.png 1x, icon@2x.png 2x, icon@3x.png 3x"
     alt="Icon">
```

Simpler than width descriptors for fixed-size images like icons.

## Art Direction with `<picture>`

When different viewports need different crops or compositions:

```html
<picture>
  <source media="(max-width: 600px)" srcset="hero-mobile.jpg">
  <source media="(max-width: 1200px)" srcset="hero-tablet.jpg">
  <img src="hero-desktop.jpg" alt="Hero image" width="1200" height="600">
</picture>
```

**Use cases**:
- Hero images: mobile shows a tighter crop, desktop shows the full scene
- Product images: mobile shows the product, desktop shows product in context
- Any image where the important content is too small on mobile in the full crop

## Lazy Loading

```html
<!-- Below the fold: lazy load -->
<img src="photo.jpg" alt="Description" loading="lazy" width="800" height="600">

<!-- Above the fold: eager load (default) -->
<img src="hero.jpg" alt="Hero" fetchpriority="high" width="1200" height="600">
```

**Rules**:
- **Above the fold**: No lazy loading. Add `fetchpriority="high"` for the LCP image.
- **Below the fold**: `loading="lazy"`. Browser loads when near viewport.
- **Always set `width` and `height`**: Prevents layout shift (CLS) while image loads.

## Aspect Ratio

Prevent layout shift by reserving space before the image loads:

```css
/* Modern: CSS aspect-ratio */
.image-container {
  aspect-ratio: 16 / 9;
  width: 100%;
}

/* With background-image */
.hero {
  aspect-ratio: 21 / 9;
  background-image: url('hero.jpg');
  background-size: cover;
  background-position: center;
}
```

**Or with HTML**: `width` and `height` attributes on `<img>` automatically set the aspect ratio in modern browsers.

## Image Optimization

### Rules

1. **Compress before serving**: Use tools like Squoosh, Sharp, ImageOptim, or build plugins
2. **Max dimensions match display size**: Don't serve a 4000px image for a 400px display slot
3. **Strip metadata**: EXIF data can add 50KB+ with no visual benefit (except when you need it)
4. **Use progressive JPEG**: Renders progressively (low-res → full-res) instead of top-to-bottom
5. **Use interlaced PNG**: Same progressive rendering for PNG

### Build-Time Optimization

```javascript
// Sharp (Node.js) — resize and convert
import sharp from 'sharp';

await sharp('input.jpg')
  .resize(800, 600, { fit: 'cover' })
  .webp({ quality: 80 })
  .toFile('output.webp');
```

### CDN-Based Optimization

Many CDNs (Cloudflare, imgix, Cloudinary) can transform images on the fly:

```
https://cdn.example.com/photo.jpg?width=800&format=webp&quality=80
```

**Benefit**: One source image, all variants generated on demand and cached.

## Video

```html
<video controls
       poster="thumbnail.jpg"
       width="1280" height="720"
       preload="metadata"
       muted>
  <source src="video.mp4" type="video/mp4">
  <source src="video.webm" type="video/webm">
  Your browser doesn't support video.
</video>
```

### Key Attributes

| Attribute | What it does |
|-----------|-------------|
| `controls` | Show browser video controls |
| `poster` | Image shown before video plays |
| `preload="metadata"` | Load only metadata (duration, dimensions), not the full video |
| `muted` | Required for autoplay in most browsers |
| `autoplay` | Auto-play (only works with `muted`) |
| `playsinline` | Play inline on iOS (not fullscreen) |
| `loop` | Loop the video |

### Video Optimization

- Use MP4 (H.264) for maximum compatibility, WebM (VP9/AV1) for better compression
- Compress with HandBrake or FFmpeg: `ffmpeg -i input.mp4 -c:v libx264 -crf 28 -preset slow output.mp4`
- Short videos (<10s): consider animated WebP or CSS animation instead
- Long videos: use adaptive streaming (HLS/DASH) instead of a single file

## SVG

### When to Use SVG

- Icons and icon systems
- Logos and brand marks
- Illustrations and diagrams
- Any graphic that needs to scale without quality loss

### SVG Optimization

```bash
# SVGO: removes metadata, unused attributes, and optimizes paths
npx svgo input.svg -o output.svg
```

### SVG in HTML

```html
<!-- As an image: no interactivity, no CSS styling from outside -->
<img src="icon.svg" alt="Search" width="24" height="24">

<!-- Inline: full CSS and JS control, but adds to DOM size -->
<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor">
  <circle cx="11" cy="11" r="8"/>
  <line x1="21" y1="21" x2="16.65" y2="16.65"/>
</svg>

<!-- As CSS background: no interactivity -->
.icon { background-image: url('icon.svg'); }
```

**Rule**: Use `<img>` for simple icons. Use inline SVG only when you need CSS styling or JS interaction.

## Windows-Specificamac

### Windows Image Processing Tools
- **Sharp (Node.js)**: Works on Windows but requires Visual C++ Redistributable
- **ImageMagick**: Available for Windows. Add to PATH or use full path
- **FFmpeg**: Essential for video processing on Windows

```powershell
# Windows: Install FFmpeg via winget
winget install Gyan.FFmpeg

# Or via chocolatey
choco install ffmpeg
```

### Windows File Paths in Build Scripts
When referencing image paths in build scripts on Windows:
```javascript
// Vite/Webpack handle paths cross-platform, but custom scripts may need:
import path from 'path';

const imagePath = path.join(__dirname, 'assets', 'images', 'hero.jpg');
// Produces: assets\images\hero.jpg on Windows
// Use path.posix.join for forward slashes if needed
```

### Windows High DPI Displays
Windows handles DPI scaling differently than macOS:
- Windows may scale images at 125%, 150%, 175%, or 200%
- Provide `srcset` with enough resolution for 200%+ scaling
- Test on actual Windows devices, not just browser emulation

## Checklist

- [ ] Images served in modern formats (WebP/AVIF) with fallbacks
- [ ] `srcset` and `sizes` used for resolution switching
- [ ] `<picture>` used for art direction (different crops per viewport)
- [ ] Above-the-fold images loaded eagerly with `fetchpriority="high"`
- [ ] Below-the-fold images use `loading="lazy"`
- [ ] `width` and `height` set on all images (prevents CLS)
- [ ] Images compressed and sized to display dimensions (not oversized)
- [ ] SVGs optimized with SVGO
- [ ] Videos have poster images and `preload="metadata"`
