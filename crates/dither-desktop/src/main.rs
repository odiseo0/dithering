#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![forbid(unsafe_code)]

mod components;
mod shell;

fn main() -> eframe::Result {
    shell::run()
}
