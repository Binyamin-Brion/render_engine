/// Textures that are used as a substitute if certain types of textures a model uses could not be loaded
pub const ERROR_TEXTURE_COLOURS: [[u8; 4]; 6] =
    [
        [0, 0, 255, 0], // Diffuse
        [0, 255, 0, 255], // Dissolve
        [0, 255, 255, 255], // Normal
        [255, 0, 0, 255], // Shininess
        [255, 0, 255, 255], // Specular
        [255, 255, 0, 255], // NoSuitableTextureStorage
    ];

pub const NO_SUITABLE_TEXTURE_STORAGE_INDEX: i32 = 7;