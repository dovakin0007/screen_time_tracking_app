CREATE TABLE window_activity_usage (
    id TEXT PRIMARY KEY, -- Unique identifier for each usage record, now a primary key
    session_id TEXT NOT NULL, -- Foreign key to sessions.id
    app_time_id TEXT NOT NULL, -- Foreign key to app_times.id
    application_name TEXT NOT NULL, -- Foreign key to apps.name
    current_screen_title TEXT NOT NULL,
    start_time TIMESTAMP NOT NULL,
    last_updated_time TIMESTAMP NOT NULL,
    FOREIGN KEY (application_name) REFERENCES apps (name),
    FOREIGN KEY (session_id) REFERENCES sessions (id),
    FOREIGN KEY (app_time_id) REFERENCES app_usage_time_period (id)
);

CREATE INDEX idx_app_usages_start_time ON window_activity_usage (start_time);
CREATE INDEX idx_app_usages_last_updated_time ON window_activity_usage (last_updated_time);
