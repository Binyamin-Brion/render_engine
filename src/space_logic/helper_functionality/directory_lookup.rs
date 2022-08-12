use std::path::{Path, PathBuf};
use render_engine::helper_things::environment::get_root_directory;

pub fn get_model_dir() -> PathBuf
{
    get_asset_dir().join("models")
}

pub fn get_skybox_texture_dir() -> PathBuf
{
    get_asset_dir().join("skybox_textures")
}

pub fn get_model_texture_dir() -> PathBuf
{
    get_asset_dir().join("model_textures/")
}

pub fn get_asset_dir() -> PathBuf
{
    let asset_dir_name = "space_game_assets";

    if Path::exists(asset_dir_name.as_ref())
    {
        Path::new(asset_dir_name).to_path_buf()
    }
    else
    {
        let mut path = get_root_directory();
        path.push("space_game_assets");
        path
    }
}