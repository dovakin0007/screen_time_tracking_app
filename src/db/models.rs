use crate::db::schema::{app, app_usage};
use chrono::{NaiveDate, NaiveDateTime};
use diesel::prelude::*;

// Model for the `app` table
#[derive(Queryable, Identifiable, Selectable, Debug, PartialEq, Default, Eq)]
#[diesel(table_name = app)]
pub struct App {
    pub id: i32,                  // Auto-incremented ID
    pub app_name: String,         // Name of the app
    pub app_path: Option<String>, // Path of the app (optional)
}

// Insertable struct for inserting new apps
#[derive(Insertable, Debug)]
#[diesel(table_name = app)]
pub struct NewApp {
    pub app_name: String,
    pub app_path: Option<String>,
}

// Model for the `app_usage` table
#[derive(Queryable, Identifiable, Selectable, Associations, Debug, PartialEq, QueryableByName)]
#[diesel(belongs_to(App, foreign_key = app_name))] // Foreign key to `App`
#[diesel(table_name = app_usage)]
pub struct AppUsage {
    pub id: i32,
    pub session_id: String,
    pub app_name: String,
    pub screen_title_name: String,
    pub duration_in_seconds: i32,
    pub is_active: i32,
    pub last_active_time: Option<NaiveDateTime>,
    pub date: NaiveDate,
    pub time_stamp: NaiveDateTime,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Queryable, Identifiable, Selectable, Associations, Debug, PartialEq, QueryableByName)]
#[diesel(belongs_to(App, foreign_key = app_name))] // Foreign key to `App`
#[diesel(table_name = app_usage)]
pub struct AppUsage2 {
    pub id: i32,
    pub session_id: String,
    pub app_name: String,
    pub screen_title_name: String,
    pub duration_in_seconds: i32,
    pub is_active: i32,
}

// Insertable struct for `app_usage`
// `date` is excluded because it will not be inserted
#[derive(Insertable, Queryable, Selectable, Associations, Debug, PartialEq, QueryableByName)]
#[diesel(belongs_to(App, foreign_key = app_name))]
#[diesel(table_name = app_usage)]
pub struct NewAppUsage {
    pub session_id: String,
    pub app_name: String,
    pub screen_title_name: String,
    pub duration_in_seconds: i32,
    pub is_active: i32,
}
