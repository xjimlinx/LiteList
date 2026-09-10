#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod model;
mod panels;
mod reminders;
mod render;
mod storage;
mod ui;

fn main() {
    if let Err(error) = ui::run() {
        ui::error_box(&format!("LiteList 无法启动：\n{error}"));
    }
}
