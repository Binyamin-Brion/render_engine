use render_engine::exports::load_models::UserLoadSkyBoxModels;
use crate::space_logic::helper_functionality::directory_lookup::get_skybox_texture_dir;

pub fn create_space_skybox() -> UserLoadSkyBoxModels
{
    UserLoadSkyBoxModels
    {
        sky_box_name: "skyBox".to_string(),
        textures: vec!
        [
            get_skybox_texture_dir().join("space_right.jpg"),
            get_skybox_texture_dir().join("space_left.jpg"),
            get_skybox_texture_dir().join("space_up.jpg"),
            get_skybox_texture_dir().join("space_down.jpg"),
            get_skybox_texture_dir().join("space_front.jpg"),
            get_skybox_texture_dir().join("space_back.jpg"),
        ]
    }
}