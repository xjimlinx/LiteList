#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod model;
mod storage;
mod render;
mod ui;
mod reminders;
mod panels;

fn main() {
    if let Err(error) = ui::run() {
        ui::error_box(&format!("LiteList 无法启动：\n{error}"));
    }
}
