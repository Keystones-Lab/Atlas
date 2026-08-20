// Prevents additional console window on Windows in release, DO NOT REMOVE!!(after build in the .exe file)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// this is the main function call when starting the application
fn main() {
    files_lib::run();
}
