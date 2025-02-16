CREATE TABLE total_app_usage_time (
    id Text PRIMARY KEY,
    app_name TEXT NOT NULL,
    start_time DATETIME NOT NULL,
    end_time DATETIME,
    FOREIGN KEY (app_name) REFERENCES apps(name) ON DELETE CASCADE
);

CREATE INDEX idx_app_time_app_name ON total_app_usage_time(app_name);