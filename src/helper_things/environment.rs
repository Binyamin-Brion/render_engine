use std::{env, fs};
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
use
{
    std::os::unix::prelude::OsStringExt
};

/// Get the location of the asset folder
#[allow(dead_code)]
pub fn get_asset_folder() -> PathBuf
{
    get_root_directory().join("render_engine_assets")
}

/// Get the location of the model folder
#[allow(dead_code)]
pub fn get_model_folder() -> PathBuf
{
    get_asset_folder().join("models")
}

/// Get the location of the debug / playback folder
#[allow(dead_code)]
pub fn get_debug_logs_folder() -> PathBuf
{
    let path_directory = get_root_directory().join("debug_logs");
    if !Path::exists(&*path_directory)
    {
        fs::create_dir(path_directory.clone())
            .unwrap_or_else(|e| panic!("Failed to create debug logs folder: {}", e));
    }
    path_directory
}

/// Get the location of the folders holding the generated shaders
pub fn get_generated_shaders_folder() -> PathBuf
{
    println!("Attempting to find: {:?}", get_root_directory());
    let path_directory = get_root_directory().join("generated_shaders");
    if !Path::exists(&*path_directory)
    {
        fs::create_dir(path_directory.clone())
            .unwrap_or_else(|e| panic!("Failed to create generated shaders folder: {}", e));
    }
    path_directory
}

/// Get the root directory of the project
#[cfg(debug_assertions)]
pub fn get_root_directory() -> PathBuf
{
    // TODO: This logic of finding the root folder might need to be fixed when the created exe is
    // TODO: not in a project "target" folder

    // This function should work from within an IDE and when directly launching the debug executable.
    // Using current_dir() won't work when launching the executable directly

    let mut path = env::current_exe().unwrap_or_else(|err| panic!("Failed to get executable directory: {}", err));

    path = PathBuf::from(path.parent().unwrap_or_else(|| panic!("Failed to get to 'debug/release directory' from executable")));
    path = PathBuf::from(path.parent().unwrap_or_else(|| panic!("Failed to get to 'target' from executable")));
    path = PathBuf::from(path.parent().unwrap_or_else(|| panic!("Failed to get to 'root' directory from executable")));
    path = PathBuf::from(path.parent().unwrap_or_else(|| panic!("Failed to get to 'parent-root'' directory from executable")));

    path.join("render_engine").to_path_buf()
}

#[cfg(not(debug_assertions))]
pub fn get_root_directory() -> PathBuf
{
    std::env::current_dir().unwrap_or_else(|_| panic!("Failed to get path of executable"))
}

/// Converts a path to byes so that it can be loaded by C libraries
#[cfg(target_os = "linux")]
pub fn path_to_bytes(path: PathBuf) -> Vec<u8>
{
    path.as_os_str().to_owned().into_vec()
}

/// Converts a path to byes so that it can be loaded by C libraries
#[cfg(target_os = "windows")]
pub fn path_to_bytes<A: AsRef<Path>>(path: A) -> Vec<u8>
{
    path.as_ref().to_path_buf().as_os_str().to_str().unwrap().as_bytes().to_owned()
}