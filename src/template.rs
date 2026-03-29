use std::fmt::Write;

use crate::db::{App, TagColor};

/// Escape a string for safe inclusion in HTML text and double-quoted attributes.
fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

/// Parse `#RRGGBB` into `(r, g, b)`. Returns `None` on invalid input.
fn hex_to_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

#[allow(clippy::too_many_lines)]
pub fn render(
    alive_ports: &[u16],
    apps: &[App],
    scan_start: u16,
    scan_end: u16,
    dashboard_port: u16,
    tag_colors: &[TagColor],
) -> String {
    let rows = build_rows(alive_ports, apps);
    let total = rows.0;
    let plural = if total == 1 { "" } else { "s" };

    let content = if rows.1.is_empty() {
        r#"<p class="empty">No active ports found.</p>"#.to_string()
    } else {
        format!("<table>{}</table>", rows.1)
    };

    // Collect unique categories from apps for dynamic filter buttons
    let mut categories: Vec<&str> = apps
        .iter()
        .map(|a| a.category.as_str())
        .filter(|c| !c.is_empty())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    categories.sort_unstable();

    let mut filter_btns = String::from(
        r#"<button class="filter active" onclick="filterBy('all', this)">all</button>"#,
    );
    for cat in &categories {
        let cat_esc = html_escape(cat);
        let _ = write!(
            filter_btns,
            r#"<button class="filter" data-category="{cat_esc}" onclick="filterBy('{cat_esc}', this)">{cat_esc}</button>"#,
        );
    }

    // Generate dynamic CSS for custom tag colors
    let mut custom_css = String::new();
    for tc in tag_colors {
        let css_class: String = tc
            .category
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
            .collect();
        if let Some((r, g, b)) = hex_to_rgb(&tc.color) {
            let cat_esc = html_escape(&tc.category);
            let _ = write!(
                custom_css,
                r#"
  .badge.badge-{css_class} {{
    background: rgba({r}, {g}, {b}, 0.08);
    color: rgba({r}, {g}, {b}, 0.7);
    border: 1px solid rgba({r}, {g}, {b}, 0.1);
  }}
  .filter[data-category="{cat_esc}"] {{
    border-color: rgba({r}, {g}, {b}, 0.2);
  }}
  .filter[data-category="{cat_esc}"].active {{
    background: rgba({r}, {g}, {b}, 0.12);
    color: rgba({r}, {g}, {b}, 0.8);
    border-color: rgba({r}, {g}, {b}, 0.3);
  }}"#
            );
        }
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8" />
<link rel="icon" href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><text y='.88em' font-size='80' fill='%23999'>◎</text></svg>" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>portmap</title>
<link rel="preconnect" href="https://fonts.googleapis.com" />
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&display=swap" rel="stylesheet" />
<style>
{CSS}
{custom_css}
</style>
</head>
<body>
  <svg class="noise-svg" aria-hidden="true"><filter id="grain"><feTurbulence type="fractalNoise" baseFrequency="0.8" numOctaves="4" stitchTiles="stitch"/></filter><rect width="100%" height="100%" filter="url(#grain)"/></svg>
  <div class="shell">
    <nav>
      <div class="nav-left">
        <span class="logo">&#x25ce;</span>
        <h1>portmap</h1>
        <span class="pill">{total} port{plural}</span>
      </div>
      <div class="nav-right">
        <span class="meta">{scan_start}&ndash;{scan_end}</span>
        <button class="btn" onclick="location.reload()">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.2"/></svg>
        </button>
      </div>
    </nav>
    <div class="filters">{filter_btns}</div>
    <div class="card">
      {content}
    </div>
    <div class="links">
      <a href="/api/ports">json</a>
      <a href="/api/apps">apps</a>
      <a href="/markdown">markdown</a>
      <span class="links-port">:{dashboard_port}</span>
    </div>
  </div>
  <div id="color-menu" style="display:none">
    <div class="color-grid"></div>
    <button class="color-reset">reset</button>
  </div>
{SCRIPT}
</body>
</html>"#
    )
}

/// Returns `(count, html_rows)`.
fn build_rows(alive_ports: &[u16], apps: &[App]) -> (usize, String) {
    let mut rows = String::new();
    let mut count = 0;

    let mut macos_rows = String::new();
    let mut macos_count = 0;

    // Alive ports: registered and unregistered (non-macOS) first
    for &port in alive_ports {
        let app = apps.iter().find(|a| a.port == i64::from(port));
        let known = crate::known_ports::lookup(port);

        if let Some(a) = app {
            count += 1;
            write_row(&mut rows, port, &a.name, &a.category, a.id, true);
        } else if let Some(k) = known {
            macos_count += 1;
            write_row(&mut macos_rows, port, k.name, "macos", 0, true);
        } else {
            count += 1;
            write_row(&mut rows, port, "", "", 0, true);
        }
    }

    // Registered but down apps
    for app in apps {
        let port = u16::try_from(app.port).unwrap_or(0);
        if alive_ports.contains(&port) {
            continue;
        }
        count += 1;
        write_row(&mut rows, port, &app.name, &app.category, app.id, false);
    }

    // macOS system ports at the bottom
    count += macos_count;
    rows.push_str(&macos_rows);

    (count, rows)
}

fn write_row(rows: &mut String, port: u16, name: &str, category: &str, app_id: i64, alive: bool) {
    let name_esc = html_escape(name);
    let cat_esc = html_escape(category);

    let display_name = if name.is_empty() {
        format!(r#"<span class="unnamed">{port}</span>"#)
    } else {
        name_esc.clone()
    };
    let badge = category_badge(&cat_esc);
    let status = if alive { "alive" } else { "down" };
    let row_class = if alive { "row" } else { "row is-down" };

    let name_val = if name.is_empty() {
        String::new()
    } else {
        name_esc.clone()
    };

    let edit_btn = r#"<button class="edit-btn" onclick="event.stopPropagation();inlineEdit(event, this.closest('.row'))" title="Edit"><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"/></svg></button>"#;
    let delete_btn = if app_id > 0 {
        format!(
            r#"<button class="del" onclick="event.stopPropagation();deleteApp({app_id})">&times;</button>"#
        )
    } else {
        String::new()
    };

    let _ = write!(
        rows,
        r#"
        <tr class="{row_class}" data-port="{port}" data-app-id="{app_id}" data-name="{name_val}" data-category="{cat_esc}"
            onclick="go({port})" oncontextmenu="inlineEdit(event, this)">
          <td class="c-status"><span class="dot {status}"></span></td>
          <td class="c-name">
            <span class="c-name-text">{display_name}</span>
            <input class="inline-input" data-field="name" value="{name_val}" placeholder="name" style="display:none" />
          </td>
          <td class="c-badge">
            <span class="c-badge-text">{badge}</span>
            <input class="inline-input cat-inline" data-field="category" value="{cat_esc}" placeholder="tag" style="display:none" />
          </td>
          <td class="c-port">{port}</td>
          <td class="c-del">{edit_btn}{delete_btn}</td>
        </tr>"#,
    );
}

/// Expects a pre-escaped category string for the display text.
/// The CSS class uses only alphanumeric/hyphen characters.
fn category_badge(category: &str) -> String {
    if category.is_empty() {
        return String::new();
    }
    let css_class: String = category
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    format!(r#"<span class="badge badge-{css_class}">{category}</span>"#)
}

const CSS: &str = r"
  * { margin: 0; padding: 0; box-sizing: border-box; }

  body {
    font-family: 'Inter', -apple-system, BlinkMacSystemFont, sans-serif;
    background: #09090b;
    color: #d4d4d4;
    min-height: 100vh;
    display: flex;
    justify-content: center;
    padding: 2rem 1rem;
    background-image:
      radial-gradient(ellipse 80% 50% at 50% -20%, rgba(120, 80, 255, 0.15), transparent),
      radial-gradient(ellipse 60% 40% at 80% 100%, rgba(34, 197, 94, 0.08), transparent);
  }

  .noise-svg {
    position: fixed;
    inset: 0;
    width: 100%;
    height: 100%;
    z-index: 0;
    pointer-events: none;
    opacity: 0.03;
  }

  body > :not(.noise-svg) { position: relative; z-index: 1; }

  .shell {
    width: 100%;
    max-width: 720px;
  }

  nav {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.75rem;
    padding: 0 0.25rem;
  }

  .nav-left {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .logo {
    font-size: 1rem;
    color: #555;
  }

  h1 {
    font-size: 0.9rem;
    font-weight: 600;
    color: #999;
    letter-spacing: -0.01em;
  }

  .pill {
    font-size: 0.65rem;
    color: #555;
    background: rgba(255,255,255,0.04);
    padding: 0.15rem 0.5rem;
    border-radius: 9999px;
    border: 1px solid rgba(255,255,255,0.06);
  }

  .nav-right {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .meta {
    font-size: 0.65rem;
    color: #3a3a3a;
  }

  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.35rem;
    padding: 0.3rem 0.5rem;
    background: rgba(255,255,255,0.04);
    border: 1px solid rgba(255,255,255,0.07);
    color: #666;
    font-family: inherit;
    font-size: 0.7rem;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.12s;
  }

  .btn:hover {
    background: rgba(255,255,255,0.08);
    color: #aaa;
    border-color: rgba(255,255,255,0.12);
  }

  .card {
    background: linear-gradient(180deg, rgba(255,255,255,0.03) 0%, rgba(255,255,255,0.01) 100%);
    border: 1px solid rgba(255,255,255,0.06);
    border-radius: 10px;
    overflow: auto;
    backdrop-filter: blur(8px);
    max-height: 75vh;
  }

  table { width: 100%; border-collapse: collapse; }

  .row {
    border-bottom: 1px solid rgba(255,255,255,0.04);
    cursor: pointer;
    transition: background 0.1s;
  }

  .row:last-child { border-bottom: none; }

  .row:hover {
    background: rgba(255,255,255,0.03);
  }

  .row.is-down {
    opacity: 0.35;
  }

  .row.is-down:hover {
    opacity: 0.55;
  }

  td {
    padding: 0.65rem 0.75rem;
    vertical-align: middle;
  }

  .c-status {
    width: 28px;
    padding-left: 0.75rem;
    vertical-align: middle;
    line-height: 0;
  }

  .dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }

  .dot.alive {
    background: #22c55e;
    box-shadow: 0 0 5px rgba(34, 197, 94, 0.35);
  }

  .dot.down {
    background: #333;
  }

  .c-name {
    font-size: 0.875rem;
    font-weight: 500;
    color: #ccc;
  }

  .unnamed {
    color: #3a3a3a;
    font-weight: 400;
  }

  .c-badge { width: 100px; }

  .badge {
    display: inline-block;
    font-size: 0.65rem;
    font-weight: 500;
    padding: 0.1rem 0.45rem;
    border-radius: 4px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    background: rgba(200, 200, 200, 0.08);
    color: rgba(200, 200, 200, 0.7);
    border: 1px solid rgba(200, 200, 200, 0.1);
  }

  .badge.badge-frontend {
    background: rgba(56, 189, 248, 0.08);
    color: rgba(56, 189, 248, 0.7);
    border: 1px solid rgba(56, 189, 248, 0.1);
  }

  .badge.badge-backend {
    background: rgba(74, 222, 128, 0.08);
    color: rgba(74, 222, 128, 0.7);
    border: 1px solid rgba(74, 222, 128, 0.1);
  }

  .badge.badge-mcp {
    background: rgba(168, 85, 247, 0.08);
    color: rgba(168, 85, 247, 0.7);
    border: 1px solid rgba(168, 85, 247, 0.1);
  }

  .badge.badge-macos {
    background: rgba(148, 148, 148, 0.08);
    color: rgba(148, 148, 148, 0.6);
    border: 1px solid rgba(148, 148, 148, 0.1);
  }

  .cat-inline {
    width: 80px;
  }

  .c-port {
    text-align: right;
    font-size: 0.8rem;
    color: #555;
    font-variant-numeric: tabular-nums;
    padding-right: 0.4rem;
  }

  .c-del {
    width: 50px;
    text-align: center;
    padding-right: 0.6rem;
    white-space: nowrap;
    vertical-align: middle;
    line-height: 0;
  }

  .c-del .edit-btn + .del { margin-left: 4px; }

  .edit-btn {
    background: none;
    border: none;
    color: transparent;
    cursor: pointer;
    padding: 0;
    line-height: 1;
    transition: color 0.1s;
    vertical-align: middle;
  }
  .row:hover .edit-btn { color: #666; }
  .edit-btn:hover { color: #ccc !important; }

  .del {
    background: none;
    border: none;
    color: transparent;
    font-size: 0.85rem;
    cursor: pointer;
    padding: 0;
    line-height: 1;
    transition: color 0.1s;
    vertical-align: middle;
  }

  .row:hover .del { color: #666; }
  .del:hover { color: #ef4444 !important; }

  .links {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.6rem 0.25rem;
  }

  .links a {
    color: #555;
    text-decoration: none;
    font-size: 0.65rem;
    transition: color 0.12s;
  }

  .links a:hover { color: #999; }

  .links-port {
    margin-left: auto;
    color: #444;
    font-size: 0.65rem;
  }

  .filters {
    display: flex;
    gap: 0.35rem;
    margin-bottom: 0.5rem;
    padding: 0 0.1rem;
  }

  .filter {
    font-family: inherit;
    font-size: 0.75rem;
    font-weight: 500;
    padding: 0.2rem 0.6rem;
    border-radius: 5px;
    border: 1px solid rgba(255,255,255,0.06);
    background: rgba(255,255,255,0.02);
    color: #555;
    cursor: pointer;
    transition: all 0.12s;
    text-transform: lowercase;
  }

  .filter:hover {
    background: rgba(255,255,255,0.05);
    color: #888;
    border-color: rgba(255,255,255,0.1);
  }

  .filter.active {
    background: rgba(255,255,255,0.08);
    color: #ccc;
    border-color: rgba(255,255,255,0.12);
  }

  .filter[data-category='frontend'] { border-color: rgba(56, 189, 248, 0.2); }
  .filter[data-category='frontend'].active { background: rgba(56, 189, 248, 0.12); color: rgba(56, 189, 248, 0.8); border-color: rgba(56, 189, 248, 0.3); }

  .filter[data-category='backend'] { border-color: rgba(74, 222, 128, 0.2); }
  .filter[data-category='backend'].active { background: rgba(74, 222, 128, 0.12); color: rgba(74, 222, 128, 0.8); border-color: rgba(74, 222, 128, 0.3); }

  .filter[data-category='mcp'] { border-color: rgba(168, 85, 247, 0.2); }
  .filter[data-category='mcp'].active { background: rgba(168, 85, 247, 0.12); color: rgba(168, 85, 247, 0.8); border-color: rgba(168, 85, 247, 0.3); }

  .empty {
    text-align: center;
    color: #444;
    padding: 2rem 1rem;
    font-size: 0.8rem;
  }

  .row.editing {
    background: linear-gradient(90deg, rgba(139, 92, 246, 0.08) 0%, rgba(139, 92, 246, 0.03) 100%);
    box-shadow: inset 3px 0 0 rgba(139, 92, 246, 0.5);
  }

  .row.editing .c-name-text,
  .row.editing .c-badge-text { display: none; }

  .row.editing .inline-input { display: inline-block !important; }

  .inline-input {
    background: rgba(255,255,255,0.04);
    border: 1px solid rgba(255,255,255,0.1);
    color: #e0e0e0;
    font-family: inherit;
    font-size: 0.75rem;
    padding: 0.2rem 0.45rem;
    border-radius: 5px;
    outline: none;
    width: 100%;
    transition: border-color 0.12s;
  }

  .inline-input:focus {
    border-color: rgba(255,255,255,0.25);
  }

  #color-menu {
    position: fixed;
    z-index: 100;
    background: #1a1a1e;
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 8px;
    padding: 0.5rem;
    box-shadow: 0 8px 24px rgba(0,0,0,0.5);
  }

  .color-grid {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 4px;
  }

  .color-swatch {
    width: 22px;
    height: 22px;
    border-radius: 4px;
    border: 1px solid rgba(255,255,255,0.1);
    cursor: pointer;
    transition: transform 0.1s, border-color 0.1s;
  }

  .color-swatch:hover {
    transform: scale(1.2);
    border-color: rgba(255,255,255,0.3);
  }

  .color-reset {
    width: 100%;
    margin-top: 4px;
    background: none;
    border: 1px solid rgba(255,255,255,0.06);
    color: #666;
    font-family: inherit;
    font-size: 0.6rem;
    padding: 0.15rem;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.1s;
  }

  .color-reset:hover {
    background: rgba(255,255,255,0.05);
    color: #999;
  }
";

#[allow(clippy::needless_raw_string_hashes)]
const SCRIPT: &str = r#"
<script>
let editingRow = null;

function go(port) {
  if (editingRow) return;
  window.open('http://localhost:' + port, '_blank');
}

function inlineEdit(e, row) {
  e.preventDefault();
  if (editingRow === row) return;
  if (editingRow) cancelEdit();

  editingRow = row;
  row.classList.add('editing');
  const nameInput = row.querySelector('[data-field="name"]');
  const catInput = row.querySelector('[data-field="category"]');
  nameInput.value = row.dataset.name || '';
  catInput.value = row.dataset.category || '';
  nameInput.focus();
  nameInput.select();
}

function cancelEdit() {
  if (!editingRow) return;
  editingRow.classList.remove('editing');
  editingRow = null;
}

async function saveEdit(row) {
  const port = parseInt(row.dataset.port);
  const appId = parseInt(row.dataset.appId);
  const name = row.querySelector('[data-field="name"]').value.trim();
  const category = row.querySelector('[data-field="category"]').value.trim();

  if (appId > 0) {
    await fetch(`/api/apps/${appId}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name: name || null, category: category || null })
    });
  } else if (name || category) {
    await fetch('/api/apps', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name: name || null, port, category: category || 'other' })
    });
  }

  location.reload();
}

function filterBy(cat, btn) {
  document.querySelectorAll('.filter').forEach(b => b.classList.remove('active'));
  btn.classList.add('active');
  document.querySelectorAll('.row').forEach(row => {
    if (cat === 'all') {
      row.style.display = '';
    } else {
      const badge = row.querySelector('.badge');
      const rowCat = badge ? badge.textContent.trim() : '';
      row.style.display = (rowCat === cat) ? '' : 'none';
    }
  });
}

async function deleteApp(appId) {
  await fetch(`/api/apps/${appId}`, { method: 'DELETE' });
  location.reload();
}

// -- Color picker --

const COLOR_SWATCHES = [
  '#ef4444', '#f97316', '#eab308', '#22c55e', '#14b8a6',
  '#38bdf8', '#8b5cf6', '#ec4899', '#6b7280', '#f5f5f4'
];

let colorMenuTarget = null;

function initColorMenu() {
  const menu = document.getElementById('color-menu');
  const grid = menu.querySelector('.color-grid');
  COLOR_SWATCHES.forEach(hex => {
    const swatch = document.createElement('div');
    swatch.className = 'color-swatch';
    swatch.style.background = hex;
    swatch.dataset.color = hex;
    swatch.addEventListener('click', () => setTagColor(hex));
    grid.appendChild(swatch);
  });
  menu.querySelector('.color-reset').addEventListener('click', resetTagColor);
}

function showColorMenu(e, category) {
  e.preventDefault();
  e.stopPropagation();
  colorMenuTarget = category;
  const menu = document.getElementById('color-menu');
  menu.style.display = 'block';
  const x = Math.min(e.clientX, window.innerWidth - 140);
  const y = Math.min(e.clientY, window.innerHeight - 160);
  menu.style.left = x + 'px';
  menu.style.top = y + 'px';
}

function hideColorMenu() {
  document.getElementById('color-menu').style.display = 'none';
  colorMenuTarget = null;
}

async function setTagColor(hex) {
  if (!colorMenuTarget) return;
  await fetch(`/api/tag-colors/${encodeURIComponent(colorMenuTarget)}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ color: hex })
  });
  hideColorMenu();
  location.reload();
}

async function resetTagColor() {
  if (!colorMenuTarget) return;
  await fetch(`/api/tag-colors/${encodeURIComponent(colorMenuTarget)}`, {
    method: 'DELETE'
  });
  hideColorMenu();
  location.reload();
}

document.addEventListener('keydown', e => {
  if (!editingRow && !colorMenuTarget) return;
  if (e.key === 'Enter' && editingRow) { e.preventDefault(); saveEdit(editingRow); }
  if (e.key === 'Escape') { cancelEdit(); hideColorMenu(); }
});

document.addEventListener('click', e => {
  if (editingRow && !editingRow.contains(e.target)) cancelEdit();
  const menu = document.getElementById('color-menu');
  if (colorMenuTarget && !menu.contains(e.target)) hideColorMenu();
});

// Right-click on filter buttons to open color picker
document.addEventListener('contextmenu', e => {
  const btn = e.target.closest('.filter[data-category]');
  if (btn) {
    showColorMenu(e, btn.dataset.category);
  }
});

initColorMenu();
setTimeout(() => location.reload(), 30000);
</script>
"#;
