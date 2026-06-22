---
name: javascript-dom
description: Use when manipulating the DOM with vanilla JavaScript - selecting elements, handling events, modifying content and attributes, managing forms, and working with browser APIs without a framework. Load when a task involves writing vanilla JS for a web page, debugging DOM interaction issues, or when a framework isn't in play.
---

# JavaScript DOM

## Overview

The DOM (Document Object Model) is the browser's representation of an HTML page as a tree of objects. Vanilla JS DOM manipulation is still relevant: simple pages, progressive enhancement, browser extensions, and understanding what frameworks do under the hood. This skill covers the DOM APIs you use most, with patterns that avoid common pitfalls like XSS, memory leaks, and layout thrashing.

For HTML structure and semantics, see `html_structure.md`. For CSS styling, see `css_styling.md`. For React patterns, see `react_patterns.md`.

## Selecting Elements

### Modern Selection

```javascript
// Single element (returns first match or null)
const el = document.querySelector('.card');          // CSS selector
const el = document.querySelector('[data-role="nav"]');

// Multiple elements (returns NodeList, not live)
const items = document.querySelectorAll('.item');    // Static NodeList

// By ID (fastest, but querySelector is fine)
const header = document.getElementById('main-header');

// Closest ancestor matching selector
const card = button.closest('.card');

// Check if element matches selector
if (el.matches('.active')) { ... }
```

### NodeList vs. Array

```javascript
// NodeList doesn't have array methods — convert if needed
const items = document.querySelectorAll('.item');
items.forEach(item => console.log(item));           // forEach works on NodeList
const ids = [...items].map(item => item.id);         // Spread to array for map/filter
```

## Reading and Writing

### Text and HTML

```javascript
// textContent: safe, returns plain text
el.textContent;                    // Read
el.textContent = 'Hello';          // Write (safe — no HTML parsing)

// innerHTML: parses HTML string — XSS RISK
el.innerHTML;                      // Read (returns HTML string)
el.innerHTML = '<em>Hello</em>';   // Write (DANGEROUS with user input)

// NEVER do this:
el.innerHTML = `<div>${userInput}</div>`;  // XSS vulnerability!

// insertAdjacentHTML: safer insertion at specific positions
el.insertAdjacentHTML('beforeend', '<li>New item</li>');
// Positions: 'beforebegin', 'afterbegin', 'beforeend', 'afterend'
```

### Attributes and Properties

```javascript
// Attributes (HTML attributes)
el.getAttribute('href');
el.setAttribute('aria-expanded', 'true');
el.removeAttribute('disabled');
el.hasAttribute('data-active');

// Properties (DOM properties — often synced with attributes)
el.id;                    // From attribute
el.value;                 // Current input value (may differ from attribute)
el.checked;               // Current checkbox state
el.disabled;              // Boolean property

// Data attributes
el.dataset.userId;        // Reads data-user-id attribute (camelCase)
el.dataset.active = 'true';
```

### Classes

```javascript
el.classList.add('active');
el.classList.remove('active');
el.classList.toggle('active');
el.classList.contains('active');
el.classList.replace('old-class', 'new-class');
```

## Creating and Inserting

```javascript
// Create
const div = document.createElement('div');
div.className = 'card';
div.textContent = 'Hello';

// Insert
parent.append(div);              // End of children
parent.prepend(div);            // Beginning of children
parent.insertBefore(div, ref);  // Before specific child

// Remove
div.remove();                    // Remove from DOM
parent.removeChild(div);         // Older API

// Replace
oldElement.replaceWith(newElement);
```

### DocumentFragment (Batch Insertions)

```javascript
// Create many elements without triggering reflow for each one
const fragment = document.createDocumentFragment();
items.forEach(item => {
  const li = document.createElement('li');
  li.textContent = item.name;
  fragment.append(li);
});
list.append(fragment);  // One DOM update
```

## Event Handling

### addEventListener

```javascript
// Basic
button.addEventListener('click', handleClick);

// With options
el.addEventListener('click', handler, { once: true });     // Auto-remove after first call
el.addEventListener('click', handler, { passive: true });  // Won't call preventDefault
el.addEventListener('click', handler, { signal: ac.signal }); // AbortController for cleanup

// Remove
button.removeEventListener('click', handler);  // Needs same function reference
```

### The Event Object

```javascript
function handleClick(event) {
  event.target;           // Element that triggered the event (may be a child)
  event.currentTarget;    // Element the listener is attached to (= this)
  event.type;             // 'click'
  event.preventDefault(); // Stop default behavior (form submit, link navigation)
  event.stopPropagation(); // Stop event from bubbling up
}
```

### Event Delegation

Attach one listener to a parent instead of many to children:

```javascript
// Bad: one listener per item (100 items = 100 listeners)
items.forEach(item => {
  item.addEventListener('click', handleItemClick);
});

// Good: one listener on the parent
list.addEventListener('click', (event) => {
  const item = event.target.closest('.list-item');
  if (!item) return;  // Click wasn't on an item

  const id = item.dataset.id;
  handleItemClick(id);
});
```

**Benefits**: Fewer listeners, works for dynamically added items, lower memory usage.

## Forms

### Reading Form Values

```javascript
// Individual inputs
input.value;              // Current text value
input.checked;            // Checkbox/radio state
select.value;             // Selected option value

// FormData: all form values at once
const form = document.querySelector('form');
const data = new FormData(form);
const obj = Object.fromEntries(data);  // { name: 'Alice', email: 'a@b.com' }

// FormData with multi-select
data.getAll('tags');  // ['js', 'css', 'html']
```

### Validation

```javascript
// HTML5 Constraint Validation API
input.checkValidity();           // Returns boolean
input.reportValidity();          // Shows browser validation UI
input.validity.valid;            // Is it valid?
input.validity.valueMissing;    // Required but empty
input.validity.typeMismatch;    // Wrong type (email, url)
input.validity.tooShort;        // Below minLength

// Custom validation
input.setCustomValidity('Must be a future date');
input.setCustomValidity('');    // Clear custom error

// Prevent form submission
form.addEventListener('submit', (event) => {
  if (!form.checkValidity()) {
    event.preventDefault();
    // Show custom error UI
  }
});
```

## Browser APIs

### fetch

```javascript
const response = await fetch('/api/users', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ name: 'Alice' })
});

if (!response.ok) throw new Error(`HTTP ${response.status}`);
const data = await response.json();
```

### localStorage / sessionStorage

```javascript
localStorage.setItem('key', JSON.stringify(data));  // Store
const data = JSON.parse(localStorage.getItem('key')); // Read
localStorage.removeItem('key');                       // Delete
localStorage.clear();                                 // Clear all

// sessionStorage: same API, cleared when tab closes
```

### IntersectionObserver

```javascript
// Detect when elements enter the viewport
const observer = new IntersectionObserver((entries) => {
  entries.forEach(entry => {
    if (entry.isIntersecting) {
      entry.target.classList.add('visible');
      observer.unobserve(entry.target);  // Stop observing once visible
    }
  });
}, { threshold: 0.1 });

document.querySelectorAll('.animate-on-scroll').forEach(el => observer.observe(el));
```

### URLSearchParams

```javascript
const params = new URLSearchParams(window.location.search);
params.get('q');          // Search query
params.set('page', '2');
params.toString();        // 'q=test&page=2'
```

## Performance

### Avoid Layout Thrashing

Reading layout properties forces the browser to calculate layout. Mixing reads and writes causes thrashing:

```javascript
// Bad: read-write-read-write forces layout recalculation each time
elements.forEach(el => {
  const height = el.offsetHeight;  // Read (forces layout)
  el.style.height = height * 2 + 'px';  // Write (invalidates layout)
});

// Good: batch reads, then batch writes
const heights = elements.map(el => el.offsetHeight);  // All reads
elements.forEach((el, i) => {
  el.style.height = heights[i] * 2 + 'px';  // All writes
});
```

### requestAnimationFrame

```javascript
// For visual updates, sync with the browser's repaint cycle
function animate() {
  updatePosition();
  requestAnimationFrame(animate);  // Schedule next frame
}
requestAnimationFrame(animate);
```

### Memory Leaks

```javascript
// Leak: event listener on removed element keeps reference
function addWidget() {
  const el = document.createElement('div');
  const handler = () => console.log('clicked');
  el.addEventListener('click', handler);
  container.append(el);
  // Later: el.remove() — but handler still references el
}

// Fix: remove listener, or use AbortController
const ac = new AbortController();
el.addEventListener('click', handler, { signal: ac.signal });
// When removing: ac.abort(); — removes all listeners on this signal
```

## Windows-Specific DOM Notes

### Windows High Contrast Mode Detection
Detect and respond to Windows High Contrast Mode:

```javascript
// Check if Windows High Contrast Mode is active
const isHighContrast = window.matchMedia('(forced-colors: active)').matches;

// Listen for changes
window.matchMedia('(forced-colors: active)').addEventListener('change', (e) => {
  if (e.matches) {
    document.body.classList.add('high-contrast');
  } else {
    document.body.classList.remove('high-contrast');
  }
});
```

### Windows Touch Events
Windows tablets and 2-in-1 devices require touch event handling:

```javascript
// Detect touch support on Windows devices
const isTouchDevice = 'ontouchstart' in window || navigator.maxTouchPoints > 0;

// Handle both mouse and touch events
function handlePointerEvent(e) {
  // Pointer events work on both mouse and touch
  console.log(e.pointerType); // 'mouse', 'touch', or 'pen'
}

element.addEventListener('pointerdown', handlePointerEvent);
```

### Windows Snap Layout Detection
Detect when the window is in a snap layout:

```javascript
// Use ResizeObserver to detect snap layout changes
const resizeObserver = new ResizeObserver((entries) => {
  for (const entry of entries) {
    const { width, height } = entry.contentRect;
    // Adjust layout based on snapped size
    if (width < 600) {
      document.body.classList.add('snap-small');
    } else {
      document.body.classList.remove('snap-small');
    }
  }
});

resizeObserver.observe(document.body);
```

### Windows File System Access API
Use the File System Access API for native file operations on Windows:

```javascript
// Open file picker (Chrome/Edge on Windows)
async function openFile() {
  try {
    const [fileHandle] = await window.showOpenFilePicker();
    const file = await fileHandle.getFile();
    const contents = await file.text();
    return contents;
  } catch (err) {
    console.error('File access cancelled or failed:', err);
  }
}

// Save file
async function saveFile(contents, filename) {
  try {
    const fileHandle = await window.showSaveFilePicker({
      suggestedName: filename,
      types: [{ accept: { 'text/plain': ['.txt'] } }]
    });
    const writable = await fileHandle.createWritable();
    await writable.write(contents);
    await writable.close();
  } catch (err) {
    console.error('Save failed:', err);
  }
}
```

## Checklist

- [ ] `textContent` used instead of `innerHTML` for plain text (XSS prevention)
- [ ] Event delegation used for lists and dynamic content
- [ ] Form values read with FormData, not individual querySelectors
- [ ] Layout reads and writes batched to avoid thrashing
- [ ] Event listeners cleaned up on element removal (AbortController)
- [ ] `querySelector`/`querySelectorAll` used (not legacy methods)
- [ ] IntersectionObserver for scroll-based behavior (not scroll events)
- [ ] Windows High Contrast Mode detected and handled
- [ ] Windows touch events supported
- [ ] Windows snap layouts tested
