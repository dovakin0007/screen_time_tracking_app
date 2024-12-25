CREATE TABLE app_usages (
    id TEXT PRIMARY KEY, -- Unique identifier for each usage record, now a primary key
    session_id TEXT NOT NULL, -- Foreign key to sessions.id
    application_name TEXT NOT NULL, -- Foreign key to apps.name
    current_screen_title TEXT NOT NULL,
    start_time TIMESTAMP NOT NULL,
    last_updated_time TIMESTAMP NOT NULL,
    FOREIGN KEY (application_name) REFERENCES apps (name)
);