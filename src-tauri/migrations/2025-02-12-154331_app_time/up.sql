CREATE TABLE app_usage_time_period (
    id Text PRIMARY KEY,
    app_name TEXT NOT NULL,
    start_time DATETIME NOT NULL,
    end_time DATETIME,
    FOREIGN KEY (app_name) REFERENCES apps(name) ON DELETE CASCADE
);

CREATE INDEX idx_app_time_app_name ON app_usage_time_period(app_name);