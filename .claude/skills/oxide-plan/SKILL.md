---
name: oxide-plan
description: Generate consistent, professional HTML implementation plans using the Oxide design system. Use when creating or updating plan documents for the Oxide project.
---

## Overview

The **oxide-plan** skill provides a reusable design system and templating tools for generating professional HTML plan documents. All Oxide plans follow a consistent visual style and structure.

## When to use this

Use this skill when you need to:

- Create a new implementation plan (e.g., `docs/plan/feature-name.html`)
- Regenerate an existing plan with design consistency
- Ensure all plans follow the Oxide visual language
- Quickly generate common plan components (summary, cards, tables, code blocks)

## Design System

The design system is defined in `design-system.css` and includes:

### Color Palette

| Token | Color | Usage |
|-------|-------|-------|
| `--accent` | `#58a6ff` | Primary blue, headings, links |
| `--accent2` | `#f78166` | Orange, highlights |
| `--green` | `#3fb950` | Success, positive outcomes |
| `--yellow` | `#d29922` | Warnings, notes |
| `--red` | `#f85149` | Errors, blockers |
| `--purple` | `#bc8cff` | Info, secondary |
| `--orange` | `#db6d28` | Code highlights, alternative |

### Component Classes

#### Cards
- `.card` — Basic container (default surface style)
- `.card-success` — Green success card
- `.card-warning` — Yellow warning card
- `.card-error` — Red error card
- `.card-info` — Purple info card
- `.card-accent` — Blue accent card

#### Special Containers
- `.objective` — Objective/goal summary with accent left border
- `.summary` — Plan summary with accent left border
- `.flow-diagram` — ASCII flow diagram with monospace styling

#### Tags
- `.tag-new` — Purple tag for new features
- `.tag-mod` — Blue tag for modifications
- `.tag-dep` — Yellow tag for dependencies
- `.tag-fix` — Green tag for fixes

#### Method Badges
- `.method-get` — Blue GET badge
- `.method-post` — Orange POST badge
- `.method-put` — Yellow PUT badge
- `.method-delete` — Red DELETE badge

## Plan Template Structure

Every Oxide plan should follow this structure:

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Oxide — Feature Name</title>
  <style>
    /* Import design-system.css content here */
  </style>
</head>
<body>

<h1><span>Oxide</span> Feature Name</h1>

<div class="objective">
  <strong>Goal:</strong> Clear objective statement.<br>
  <strong>Scope:</strong> What's included / excluded.<br>
  <strong>Metrics:</strong> How success is measured.
</div>

<section>
  <h2><span class="num">1</span> Section Title</h2>
  <div class="card">
    <!-- Content -->
  </div>
</section>

<section>
  <h2><span class="num">2</span> Next Section</h2>
  <!-- More content -->
</section>

</body>
</html>
```

## Workflow

### Step 1: Define the plan scope

Before generating HTML, clarify:

- **Goal**: One clear objective
- **Scope**: What's in / out
- **Success metrics**: How to validate
- **Key decisions**: Major tradeoffs

### Step 2: Gather content

Organize sections:

1. Overview / Summary
2. Current state / Problem
3. Proposed solution / Architecture
4. Implementation details (phases, dependencies)
5. Testing strategy
6. Risks & mitigations
7. Success criteria

### Step 3: Generate HTML

Build the HTML document using the template structure and design system classes.

Embed the complete `design-system.css` inside the `<style>` tag.

### Step 4: Validate

- [ ] Plan opens in browser without errors
- [ ] Dark theme looks good (verify colors)
- [ ] Code blocks are readable (monospace, proper contrast)
- [ ] Tables render cleanly
- [ ] Responsive on narrow screens (max-width: 960px)

## Example Components

### Objective Box

```html
<div class="objective">
  <strong>Goal:</strong> Implement X feature.<br>
  <strong>Scope:</strong> CLI subcommands, auth flow, token storage.<br>
  <strong>Success:</strong> Users can authenticate and make API calls.
</div>
```

### Success Card (Green)

```html
<div class="card card-success">
  <h3>Benefits</h3>
  <ul>
    <li>Faster performance</li>
    <li>Better error handling</li>
  </ul>
</div>
```

### Warning Card (Yellow)

```html
<div class="card card-warning">
  <h3>Risks</h3>
  <ul>
    <li>May break existing APIs</li>
    <li>Requires database migration</li>
  </ul>
</div>
```

### Code Block

```html
<pre><code>const WORKOS_CLIENT_ID = "...";
const OAUTH_SCOPE = "openid profile email";</code></pre>
```

### Table

```html
<table>
  <tr><th>Column 1</th><th>Column 2</th></tr>
  <tr><td>Value</td><td>Value</td></tr>
</table>
```

### Flow Diagram

```html
<div class="flow-diagram">
<span class="highlight">oxide login</span>
  <span class="dim">|</span>
  <span class="dim">v</span>
<span class="orange">POST</span> https://api.workos.com/authorize
  <span class="dim">|</span>
  <span class="dim">v</span>
<span class="green">Success!</span>
</div>
```

## Best Practices

1. **Use semantic HTML**: Prefer `<section>`, `<h2>`, `<h3>` for structure
2. **Leverage the color system**: Use `.card-success`, `.card-warning` instead of generic classes
3. **Keep it scannable**: Short paragraphs, bullet lists, clear headings
4. **Code samples first**: Show code before prose when possible
5. **Link to related docs**: Reference `docs/adr/`, `CLAUDE.md`, related plans
6. **Validate in browser**: Always open the HTML in a dark-themed browser

## Implementation Notes

- The design system uses CSS custom properties (`:root`) for easy theming
- All colors follow GitHub's dark theme for consistency with developer tools
- Typography uses system fonts for optimal rendering
- Monospace font uses a fallback chain: SF Mono → Fira Code → JetBrains Mono
- Responsive design targets max-width: 960px (single-column layout)
