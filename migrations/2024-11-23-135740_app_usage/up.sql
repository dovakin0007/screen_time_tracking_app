-- Updated `app_usage` table
CREATE TABLE app_usage (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,           -- Unique ID for each usage entry
    session_id TEXT NOT NULL,                                 -- Session identifier (e.g., user session)
    app_name TEXT NOT NULL,                                   -- Foreign key to `app` (app_id)
    screen_title_name TEXT NOT NULL,                          -- Screen title name
    duration_in_seconds INTEGER NOT NULL CHECK (duration_in_seconds >= 0),  -- Duration in seconds
    is_active INTEGER NOT NULL DEFAULT 0,                     -- Active status: 0 (inactive), 1 (active)
    last_active_time DATETIME,                                -- Last timestamp the window was active
    date DATE NOT NULL DEFAULT (DATE('now')),                 -- Date of usage
    time_stamp DATETIME NOT NULL DEFAULT (DATETIME('now')),   -- Record creation timestamp
    created_at DATETIME DEFAULT (DATETIME('now')),            -- Creation timestamp
    updated_at DATETIME DEFAULT (DATETIME('now')),            -- Last update timestamp
    FOREIGN KEY (app_name) REFERENCES app(app_name)          -- Foreign key to `app`
);

-- Trigger to update `updated_at` timestamp on `app_usage` updates
CREATE TRIGGER update_app_usage_timestamp
AFTER UPDATE ON app_usage
FOR EACH ROW
BEGIN
    UPDATE app_usage SET updated_at = DATETIME('now') WHERE id = OLD.id;
END;
