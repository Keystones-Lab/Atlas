use sysinfo::{
    System, Disks,
};

use serde::Serialize;
#[derive(Serialize)]
struct SystemInfo {
    cpu_percent: f64,
    used_memory: u64,
    total_memory: u64,
    used_disk: u64,
    total_disk: u64,
}

#[tauri::command] 
fn get_system_info() -> SystemInfo {
    let mut sys = System::new_all();
    let mut total_space = 0;
    let mut available_space = 0;
    let mut used_space = 0;

    sys.refresh_all();
    let disks = Disks::new_with_refreshed_list();
    for disks in &disks{
        if disks.mount_point() == "C:\\" {
            total_space = disks.total_space();
            available_space = disks.available_space();
            used_space = total_space - available_space;
        }
        else{
            println!("Error while reading the sysinfo");
        }
    }
    SystemInfo {
    cpu_percent: 18.00,
    used_memory: sys.used_memory(),
    total_memory: sys.total_memory(),
    used_disk: used_space,
    total_disk: total_space,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
    //.plugin(tauri_plugin_opener::init()) -> this is not being used and called here, it can open files/URLs in other apps
    .invoke_handler(tauri::generate_handler![get_system_info])
    .run(tauri::generate_context!())
    .expect("Error while running tauri application")
} 