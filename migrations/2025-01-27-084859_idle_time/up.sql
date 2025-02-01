CREATE TABLE idle_periods (
    id TEXT PRIMARY KEY, -- Primary key, unique and NOT NULL
    app_id TEXT, -- Foreign key to apps.name
    session_id TEXT NOT NULL, -- Foreign key to sessions.id
    app_name TEXT, -- Foreign key to apps.name
    start_time DATETIME NOT NULL, -- Start time of the idle period
    end_time DATETIME NOT NULL, -- End time of the idle period
    FOREIGN KEY (app_id) REFERENCES app_usages (id) ON DELETE CASCADE, -- Ensures referential integrity with apps table
    FOREIGN KEY (session_id) REFERENCES sessions (id) ON DELETE CASCADE, -- Ensures referential integrity with sessions table
    FOREIGN KEY (app_name) REFERENCES apps (name) ON DELETE CASCADE -- Ensures referential integrity with apps table
);

-- Time-based queries (common for time ranges)
CREATE INDEX idx_idle_periods_start_end_time ON idle_periods (start_time, end_time);