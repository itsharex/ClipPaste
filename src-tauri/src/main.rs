#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use clippaste::run_app;

fn main() {
    run_app();
}
