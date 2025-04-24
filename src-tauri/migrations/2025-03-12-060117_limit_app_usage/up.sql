CREATE TABLE daily_limits (
    app_name TEXT NOT NULL UNIQUE,
    time_limit INTEGER NOT NULL,
    should_alert BOOLEAN NOT NULL DEFAULT 0,
    should_close BOOLEAN NOT NULL DEFAULT 0,
    alert_before_close BOOLEAN NOT NULL DEFAULT 1,
    alert_duration INTEGER NOT NULL DEFAULT 300,
    FOREIGN KEY (app_name) REFERENCES apps(name) ON DELETE CASCADE
);

CREATE INDEX idx_daily_limits_app_name ON daily_limits(app_name);
