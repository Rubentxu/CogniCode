# CogniCode Dashboard — Design System

> **Style Reference**: Monday.com via Refero — Vibrant organized workspace, light theme, violet accents.
> **URL**: https://styles.refero.design/style/77ee57e9-9f8e-4ec1-93f7-cc1c4b84307a

---

## Philosophy

> *"Vibrant organized workspace — like a digital desk splashed with colorful sticky notes and neatly arranged tools."*

Balances playful accents with robust functionality. Light theme with high-contrast text and a central vivid violet (`#6161ff`) for primary actions. Rounded elements soften the structured grid layout. Colorful card backgrounds create a rich, dynamic interface.

---

## Tailwind v4 Theme

```css
@import "tailwindcss";

@theme {
  /* ─── Colors ─── */
  --color-text-primary: #333333;
  --color-text-muted: #676879;
  --color-canvas: #ffffff;
  --color-surface: #f5f6f8;
  --color-brand: #6161ff;
  --color-brand-hover: #4f4fe6;
  --color-outline: #000000;
  --color-border: #d0d4e4;
  --color-nav: #535768;

  /* Accent Cards */
  --color-accent-mint: #bcfe90;
  --color-accent-lavender: #eddff7;
  --color-accent-sky: #abf0ff;
  --color-accent-sunset: #ff8940;
  --color-accent-pale: #e7ecff;
  --color-accent-ocean: #93beff;
  --color-accent-ice: #d1faff;

  /* Buttons */
  --color-btn-indigo: #9450fd;
  --color-btn-sky: #3ac9ff;
  --color-btn-teal: #2a5c40;
  --color-badge: #dbdbff;

  /* Quality Ratings */
  --color-rating-a: #00b88c;
  --color-rating-b: #94c748;
  --color-rating-c: #e6c235;
  --color-rating-d: #f08c2e;
  --color-rating-e: #e54b4b;

  /* Severity */
  --color-severity-blocker: #c62828;
  --color-severity-critical: #e53935;
  --color-severity-major: #fb8c00;
  --color-severity-minor: #43a047;
  --color-severity-info: #1e88e5;

  /* Gradients */
  --gradient-brand: linear-gradient(135deg, #6161ff 0%, #9450fd 100%);
  --gradient-vibrant: linear-gradient(90deg, #fe81e4 0%, #fda900 100%);
  --gradient-spectrum: conic-gradient(from 270deg, #8181ff 15%, #33dbdb 40%, #33d58e 55%, #ffd633 65%, #fc527d 85%, #8181ff 100%);

  /* ─── Typography ─── */
  --font-sans: 'Poppins', ui-sans-serif, system-ui, sans-serif;
  --font-mono: 'JetBrains Mono', ui-monospace, monospace;

  --text-caption: 0.75rem;
  --text-caption--line-height: 1.5;
  --text-body-sm: 0.875rem;
  --text-body-sm--line-height: 1.5;
  --text-body: 1.125rem;
  --text-body--line-height: 1.5;
  --text-body-lg: 1.375rem;
  --text-body-lg--line-height: 1.5;
  --text-heading-sm: 1.75rem;
  --text-heading-sm--line-height: 1.3;
  --text-heading: 2rem;
  --text-heading--line-height: 1.3;
  --text-heading-lg: 2.5rem;
  --text-heading-lg--line-height: 1.3;
  --text-display: 4rem;
  --text-display--line-height: 1.3;

  --font-weight-light: 300;
  --font-weight-normal: 400;
  --font-weight-medium: 500;
  --font-weight-bold: 700;

  /* ─── Spacing ─── */
  --spacing-unit: 0.5rem;
  --spacing-section: 3rem;
  --spacing-card: 1.5rem;
  --spacing-element: 0.5rem;

  /* ─── Border Radius ─── */
  --radius-sm: 0.375rem;
  --radius-md: 0.75rem;
  --radius-lg: 1rem;
  --radius-xl: 1.5rem;
  --radius-2xl: 2.5rem;
  --radius-pill: 10rem;

  /* ─── Shadows ─── */
  --shadow-card: 0 2px 48px 0 rgba(205, 208, 223, 0.4);
  --shadow-dropdown: 0 5px 45px 0 rgba(0, 0, 0, 0.15);
  --shadow-modal: 0 5px 55px 0 rgba(0, 0, 0, 0.4);
  --shadow-inset: 0 -2px 0 0 rgb(0, 0, 0) inset;
}
```

---

## Color System

### Brand Colors

| Token | Hex | Usage |
|-------|-----|-------|
| `brand` | `#6161ff` | Primary CTA buttons, active nav, key highlights |
| `brand-hover` | `#4f4fe6` | Button hover state |
| `nav` | `#535768` | Navigation text, interactive links |
| `outline` | `#000000` | Outlined button borders |
| `border` | `#d0d4e4` | Card borders, subtle separators |

### Rating Colors (SonarQube-style)

| Rating | Hex | Meaning |
|--------|-----|---------|
| **A** | `#00b88c` | Excellent — 0 technical debt |
| **B** | `#94c748` | Good — minimal debt |
| **C** | `#e6c235` | Fair — moderate debt |
| **D** | `#f08c2e` | Poor — significant debt |
| **E** | `#e54b4b` | Critical — excessive debt |

### Severity Colors

| Severity | Hex | Badge |
|----------|-----|-------|
| Blocker | `#c62828` | `BLOCKER` |
| Critical | `#e53935` | `CRITICAL` |
| Major | `#fb8c00` | `MAJOR` |
| Minor | `#43a047` | `MINOR` |
| Info | `#1e88e5` | `INFO` |

### Card Accents (for variety)

Used as background colors for feature/metric cards:

| Token | Hex | Best for |
|-------|-----|----------|
| `accent-mint` | `#bcfe90` | Coverage, green metrics |
| `accent-lavender` | `#eddff7` | Complexity, code smells |
| `accent-sky` | `#abf0ff` | Duplications, blue metrics |
| `accent-sunset` | `#ff8940` | Security, vulnerabilities |
| `accent-pale` | `#e7ecff` | General, neutral |
| `accent-ocean` | `#93beff` | Reliability |
| `accent-ice` | `#d1faff` | Maintainability |

### Surfaces

| Level | Name | Value | Usage |
|-------|------|-------|-------|
| 0 | Canvas | `#ffffff` | Page background |
| 1 | Surface | `#f5f6f8` | Cards, badges, sections |
| 2 | Accents | Various | Feature cards categorization |

---

## Typography

### Font Family

- **Primary**: Poppins (weights: 300, 400, 500, 700)
- **Mono**: JetBrains Mono (for code, file paths, metrics)

### Type Scale

| Role | Size | Weight | Line | Usage |
|------|------|--------|------|-------|
| `caption` | 12px | 400 | 1.5 | Issue counts, meta |
| `body-sm` | 14px | 400 | 1.5 | Table cells, secondary text |
| `body` | 18px | 400 | 1.5 | Paragraphs, descriptions |
| `body-lg` | 22px | 400 | 1.5 | Card highlights |
| `heading-sm` | 28px | 700 | 1.3 | Section titles |
| `heading` | 32px | 700 | 1.3 | Page titles |
| `heading-lg` | 40px | 700 | 1.3 | Dashboard headings |
| `display` | 64px | 700 | 1.3 | Rating letters (A-E) |

---

## Components

### Primary CTA Button

```html
<button class="bg-brand text-white rounded-pill font-medium
               px-6 py-3 text-base hover:bg-brand-hover
               transition-colors duration-200">
  Analyze Project
</button>
```

### Outlined Button

```html
<button class="border border-outline text-text-primary rounded-pill
               font-medium px-6 py-3 text-base
               hover:bg-surface transition-colors">
  Cancel
</button>
```

### Feature Card

```html
<div class="bg-canvas rounded-xl p-6 shadow-card">
  <h3 class="text-heading-sm font-bold text-text-primary">Quality Gate</h3>
  <p class="text-body text-text-primary mt-2">All conditions passed.</p>
</div>
```

### Metric Card (Rating)

```html
<div class="rounded-xl p-6 text-center" style="background: var(--color-accent-pale)">
  <span class="text-display font-bold" style="color: var(--color-rating-a)">A</span>
  <p class="text-caption text-text-muted mt-2">RELIABILITY</p>
</div>
```

### Issue Row

```html
<div class="flex items-center gap-3 px-4 py-3 border-b border-border
            hover:bg-surface transition-colors cursor-pointer">
  <span class="px-2 py-0.5 rounded-sm text-caption font-bold text-white
               bg-severity-major">MAJOR</span>
  <span class="text-body-sm text-text-muted font-mono">src/main.rs:42</span>
  <span class="text-body-sm text-text-primary flex-1">Unused variable 'x'</span>
  <span class="text-caption text-text-muted">2 days ago</span>
</div>
```

### Sidebar Navigation

```html
<nav class="w-64 h-screen bg-canvas border-r border-border flex flex-col">
  <div class="p-6">
    <h1 class="text-heading-sm font-bold text-brand">CogniCode</h1>
  </div>
  <div class="flex-1 px-4">
    <a href="/" class="flex items-center gap-3 px-4 py-3 rounded-lg
                        text-nav hover:bg-surface transition-colors">
      <span class="text-body-sm font-medium">Dashboard</span>
    </a>
    <a href="/issues" class="flex items-center gap-3 px-4 py-3 rounded-lg
                           text-nav hover:bg-surface transition-colors">
      <span class="text-body-sm font-medium">Issues</span>
    </a>
    <a href="/metrics" class="flex items-center gap-3 px-4 py-3 rounded-lg
                           text-nav hover:bg-surface transition-colors">
      <span class="text-body-sm font-medium">Metrics</span>
    </a>
    <a href="/quality-gate" class="flex items-center gap-3 px-4 py-3 rounded-lg
                                text-nav hover:bg-surface transition-colors">
      <span class="text-body-sm font-medium">Quality Gate</span>
    </a>
  </div>
</nav>
```

### Filter Bar

```html
<div class="flex items-center gap-3 p-4 bg-surface rounded-lg">
  <select class="border border-border rounded-md px-3 py-2 text-body-sm text-text-primary bg-canvas">
    <option>All Severities</option>
    <option>Blocker</option>
    <option>Critical</option>
    <option>Major</option>
    <option>Minor</option>
    <option>Info</option>
  </select>
  <select class="border border-border rounded-md px-3 py-2 text-body-sm text-text-primary bg-canvas">
    <option>All Categories</option>
    <option>Bug</option>
    <option>Vulnerability</option>
    <option>Code Smell</option>
  </select>
  <input type="text" placeholder="Search issues..."
         class="border border-border rounded-md px-3 py-2 text-body-sm text-text-primary bg-canvas flex-1" />
</div>
```

---

## Layout

### Shell Layout

```
┌──────────────────────────────────────────────────────┐
│  SIDEBAR          │  HEADER (sticky)                  │
│  (w-64, fixed)   │  ┌──────────────────────────────┐ │
│                   │  │ Breadcrumb / Project Selector │ │
│  Logo             │  └──────────────────────────────┘ │
│  ─────────        │                                    │
│  Dashboard  ●     │  CONTENT (scrollable)              │
│  Issues           │  ┌──────────────────────────────┐ │
│  Metrics          │  │                              │ │
│  Quality Gate     │  │  Section Gap: 48px           │ │
│  Configuration    │  │                              │ │
│                   │  ├──────────────────────────────┤ │
│                   │  │                              │ │
│                   │  │  Section Gap: 48px           │ │
│                   │  │                              │ │
│                   │  └──────────────────────────────┘ │
└──────────────────────────────────────────────────────┘
```

### Dashboard Grid

```
┌─────────────────────────────────────────────────────────────┐
│  Quality Gate Status Bar (PASSED / FAILED)                  │
├──────────┬──────────┬──────────┬──────────┬─────────────────┤
│ Rating A │ Rating A │ Rating B │ Rating C │ Technical Debt  │
│ RELIAB.  │ SECURITY │ MAINTAIN │ COVERAGE │ 2.5% / 0d       │
├──────────┴──────────┴──────────┴──────────┴─────────────────┤
│                                                               │
│  Issues by Severity (bar chart)    Issues by Category (pie)  │
│                                                               │
├───────────────────────────────────────────────────────────────┤
│  Recent Issues Table                                          │
│  ┌──────────────────────────────────────────────────────────┐│
│  │ BLOCKER │ src/main.rs:12 │ Unhandled error               ││
│  │ MAJOR   │ lib/parse:45   │ Unused parameter              ││
│  │ MINOR   │ tests/mod:3    │ Missing test                  ││
│  └──────────────────────────────────────────────────────────┘│
└───────────────────────────────────────────────────────────────┘
```

### Responsive Breakpoints

| Breakpoint | Width | Layout |
|------------|-------|--------|
| `sm` | 640px | Single column, sidebar collapsed |
| `md` | 768px | 2-column grid |
| `lg` | 1024px | Sidebar + content, 3-column grid |
| `xl` | 1280px | Full layout, 4-column grid |

---

## Do's & Don'ts

### ✅ Do

- Use `brand` (#6161ff) **only** for primary CTA buttons
- Apply `rounded-pill` (160px) to all buttons
- Use Poppins 700 for headings, 400 for body
- Apply `shadow-card` for elevated feature cards
- Use accent colors for metric card backgrounds
- Maintain `spacing-section` (48px) between major sections
- Use `spacing-element` (8px) between inline elements

### ❌ Don't

- Don't use any color other than `brand` for primary CTA backgrounds
- Don't use square buttons (< 24px border radius)
- Don't deviate from Poppins for text
- Don't overcrowd sections (keep 48px gaps)
- Don't introduce new shadow values
- Don't center-align body text (left-align preferred)
