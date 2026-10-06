#![doc = include_str!("../README.md")]

pub mod error;
pub mod model;

mod api;
mod auth;
mod client;
mod config;

pub use api::CheckinOutcome;
pub use auth::azure::authorize_url;
pub use auth::upn;
pub use client::InstattClient;
pub use error::{Error, Result};
pub use model::{
    Attendance, AttendanceSummary, Class, ClassStatus, ClassStatusCodes, Globals, ModuleAttendance,
    ModuleKey, ServerTime,
};
