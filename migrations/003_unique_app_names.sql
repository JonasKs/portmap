CREATE UNIQUE INDEX IF NOT EXISTS idx_apps_name_unique
ON apps (name) WHERE name != '';
