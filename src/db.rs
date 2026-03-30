use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct App {
    pub id: i64,
    pub name: String,
    pub port: i64,
    pub category: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateApp {
    pub name: Option<String>,
    pub port: i64,
    pub category: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateApp {
    pub name: Option<String>,
    pub port: Option<i64>,
    pub category: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TagColor {
    pub category: String,
    pub color: String,
}

#[derive(Debug, Deserialize)]
pub struct SetTagColor {
    pub color: String,
}

pub async fn init(db_path: &str) -> Result<SqlitePool, sqlx::Error> {
    let url = format!("sqlite:{db_path}?mode=rwc");
    let pool = SqlitePool::connect(&url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub async fn list_apps(pool: &SqlitePool) -> Result<Vec<App>, sqlx::Error> {
    sqlx::query_as::<_, App>(
        "SELECT id, name, port, category, created_at FROM apps ORDER BY category, name",
    )
    .fetch_all(pool)
    .await
}

pub async fn get_app(pool: &SqlitePool, id: i64) -> Result<Option<App>, sqlx::Error> {
    sqlx::query_as::<_, App>("SELECT id, name, port, category, created_at FROM apps WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn create_app(pool: &SqlitePool, app: &CreateApp) -> Result<App, sqlx::Error> {
    let name = app.name.as_deref().unwrap_or("");
    let category = app.category.as_deref().unwrap_or("other");
    sqlx::query_as::<_, App>(
        "INSERT INTO apps (name, port, category) VALUES (?, ?, ?) RETURNING id, name, port, category, created_at",
    )
    .bind(name)
    .bind(app.port)
    .bind(category)
    .fetch_one(pool)
    .await
}

pub async fn update_app(
    pool: &SqlitePool,
    id: i64,
    update: &UpdateApp,
) -> Result<Option<App>, sqlx::Error> {
    let existing = get_app(pool, id).await?;
    let Some(existing) = existing else {
        return Ok(None);
    };

    let name = update.name.as_deref().unwrap_or(&existing.name);
    let port = update.port.unwrap_or(existing.port);
    let category = update.category.as_deref().unwrap_or(&existing.category);

    sqlx::query_as::<_, App>(
        "UPDATE apps SET name = ?, port = ?, category = ? WHERE id = ? RETURNING id, name, port, category, created_at",
    )
    .bind(name)
    .bind(port)
    .bind(category)
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_app_by_name(pool: &SqlitePool, name: &str) -> Result<Option<App>, sqlx::Error> {
    sqlx::query_as::<_, App>("SELECT id, name, port, category, created_at FROM apps WHERE name = ?")
        .bind(name)
        .fetch_optional(pool)
        .await
}

pub async fn delete_app(pool: &SqlitePool, id: i64) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM apps WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn find_app_by_port(pool: &SqlitePool, port: i64) -> Result<Option<App>, sqlx::Error> {
    sqlx::query_as::<_, App>("SELECT id, name, port, category, created_at FROM apps WHERE port = ?")
        .bind(port)
        .fetch_optional(pool)
        .await
}

// -- Tag colors --

pub async fn list_tag_colors(pool: &SqlitePool) -> Result<Vec<TagColor>, sqlx::Error> {
    sqlx::query_as::<_, TagColor>("SELECT category, color FROM tag_colors ORDER BY category")
        .fetch_all(pool)
        .await
}

pub async fn set_tag_color(
    pool: &SqlitePool,
    category: &str,
    color: &str,
) -> Result<TagColor, sqlx::Error> {
    sqlx::query_as::<_, TagColor>(
        "INSERT INTO tag_colors (category, color) VALUES (?, ?) ON CONFLICT(category) DO UPDATE SET color = excluded.color RETURNING category, color",
    )
    .bind(category)
    .bind(color)
    .fetch_one(pool)
    .await
}

pub async fn delete_tag_color(pool: &SqlitePool, category: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM tag_colors WHERE category = ?")
        .bind(category)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
