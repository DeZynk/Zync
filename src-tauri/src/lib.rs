pub mod auth;
pub mod config;
pub mod error;
pub mod export;
pub mod instances;
pub mod java;
pub mod loaders;
pub mod minecraft;
pub mod mods;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            config::load_config,
            config::save_config,
            config::get_config_path,
            config::open_config_folder,
            java::detect_java_runtimes,
            java::test_java_runtime,
            java::download_java_runtime,
            instances::list_instances,
            instances::create_instance,
            instances::update_instance,
            instances::delete_instance,
            instances::open_instance_folder,
            loaders::list_loader_versions,
            minecraft::list_minecraft_versions,
            minecraft::launch_instance,
            minecraft::kill_instance,
            mods::search_content,
            mods::install_content_item,
            mods::install_modpack,
            mods::list_installed_mods,
            mods::toggle_mod,
            mods::delete_mod,
            export::scan_instance_mods_for_export,
            export::export_instance,
            auth::add_offline_account,
            auth::remove_account,
            auth::set_active_account,
            auth::start_microsoft_login,
            auth::complete_microsoft_login,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Zync");
}
