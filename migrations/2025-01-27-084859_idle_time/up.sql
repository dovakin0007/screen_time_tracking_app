CREATE TABLE app_idle_period (
    id TEXT PRIMARY KEY, -- Primary key, unique and NOT NULL
    app_id Text,
    window_id TEXT, -- Foreign key to apps.name
    session_id TEXT NOT NULL, -- Foreign key to sessions.id
    app_name TEXT, -- Foreign key to apps.name
    start_time DATETIME NOT NULL, -- Start time of the idle period
    end_time DATETIME NOT NULL, -- End time of the idle period
    FOREIGN KEY (app_id) REFERENCES total_app_usage_time (id), -- Reference to total_app_usage_time table
    FOREIGN KEY (window_id) REFERENCES window_activity_usage (id) ON DELETE CASCADE, -- Ensures referential integrity with apps table
    FOREIGN KEY (session_id) REFERENCES sessions (id) ON DELETE CASCADE, -- Ensures referential integrity with sessions table
    FOREIGN KEY (app_name) REFERENCES apps (name) ON DELETE CASCADE -- Ensures referential integrity with apps table
);

-- Time-based queries (common for time ranges)
CREATE INDEX idx_idle_periods_start_end_time ON app_idle_period (start_time, end_time);