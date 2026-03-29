use std::fmt::Write;

use crate::db::App;

pub fn render(
    alive_ports: &[u16],
    apps: &[App],
    scan_start: u16,
    scan_end: u16,
    dashboard_port: u16,
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
        .filter(|c| !c.is_empty() && *c != "other")
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    categories.sort_unstable();

    let mut filter_btns = String::from(
        r#"<button class="filter active" onclick="filterBy('all', this)">all</button>"#,
    );
    for cat in &categories {
        let _ = write!(
            filter_btns,
            r#"<button class="filter" onclick="filterBy('{cat}', this)">{cat}</button>"#,
        );
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
    let display_name = if name.is_empty() {
        format!(r#"<span class="unnamed">{port}</span>"#)
    } else {
        name.to_string()
    };
    let badge = category_badge(category);
    let status = if alive { "alive" } else { "down" };
    let row_class = if alive { "row" } else { "row is-down" };

    let name_val = if name.is_empty() { "" } else { name };

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
        <tr class="{row_class}" data-port="{port}" data-app-id="{app_id}" data-name="{name_val}" data-category="{category}"
            onclick="go({port})" oncontextmenu="inlineEdit(event, this)">
          <td class="c-status"><span class="dot {status}"></span></td>
          <td class="c-name">
            <span class="c-name-text">{display_name}</span>
            <input class="inline-input" data-field="name" value="{name_val}" placeholder="name" style="display:none" />
          </td>
          <td class="c-badge">
            <span class="c-badge-text">{badge}</span>
            <input class="inline-input cat-inline" data-field="category" value="{category}" placeholder="tag" style="display:none" />
          </td>
          <td class="c-port">{port}</td>
          <td class="c-del">{edit_btn}{delete_btn}</td>
        </tr>"#,
    );
}

fn category_badge(category: &str) -> String {
    if category.is_empty() || category == "other" {
        return String::new();
    }
    format!(r#"<span class="badge badge-{category}">{category}</span>"#)
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
  }

  .badge-frontend {
    background: rgba(56, 189, 248, 0.08);
    color: rgba(56, 189, 248, 0.7);
    border: 1px solid rgba(56, 189, 248, 0.1);
  }

  .badge-backend {
    background: rgba(74, 222, 128, 0.08);
    color: rgba(74, 222, 128, 0.7);
    border: 1px solid rgba(74, 222, 128, 0.1);
  }

  .badge-mcp {
    background: rgba(168, 85, 247, 0.08);
    color: rgba(168, 85, 247, 0.7);
    border: 1px solid rgba(168, 85, 247, 0.1);
  }

  .badge-macos {
    background: rgba(148, 148, 148, 0.08);
    color: rgba(148, 148, 148, 0.6);
    border: 1px solid rgba(148, 148, 148, 0.1);
  }

  .cat-inline {
    width: 80px;
  }

  .badge-other, .badge:not(.badge-frontend):not(.badge-backend):not(.badge-mcp) {
    background: rgba(200, 200, 200, 0.08);
    color: rgba(200, 200, 200, 0.7);
    border: 1px solid rgba(200, 200, 200, 0.1);
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
  } else if (name) {
    await fetch('/api/apps', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name, port, category: category || 'other' })
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

document.addEventListener('keydown', e => {
  if (!editingRow) return;
  if (e.key === 'Enter') { e.preventDefault(); saveEdit(editingRow); }
  if (e.key === 'Escape') cancelEdit();
});

document.addEventListener('click', e => {
  if (editingRow && !editingRow.contains(e.target)) cancelEdit();
});

setTimeout(() => location.reload(), 30000);
</script>
"#;
