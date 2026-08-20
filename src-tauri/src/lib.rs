use serde::Serialize;
#[derive(Serialize)]
struct SystemInfo {
    cpu_percent: f64,
    used_memory: u64,
    total_memory: u64,
    used_disk: u64,
    total_disk: u64,
}

// new function made for understanding the system pipeline and the dataflow in tauri
#[tauri::command]
fn hello_atlas() -> String{
    "Hello From Atlas".to_string()
}

#[tauri::command] 
fn get_system_info() -> SystemInfo {
    SystemInfo {
    cpu_percent: 18.00,
    used_memory: 39,
    total_memory: 60,
    used_disk: 20,
    total_disk: 80,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
    //.plugin(tauri_plugin_opener::init()) -> this is not being used and called here, it can open files/URLs in other apps
    .invoke_handler(tauri::generate_handler![hello_atlas, get_system_info])
    .run(tauri::generate_context!())
    .expect("Error while running tauri application")
}