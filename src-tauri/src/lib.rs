// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod api;
pub mod app;
pub mod core;
pub mod generate;
pub mod i18n;
pub mod inference;
pub mod retrieval;

pub use app::run;
