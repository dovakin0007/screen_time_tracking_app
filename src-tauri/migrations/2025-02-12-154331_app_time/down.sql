-- This file should undo anything in `up.sql`
DROP INDEX IF EXISTS idx_app_time_app_name;
DROP TABLE IF EXISTS app_usage_time_period;