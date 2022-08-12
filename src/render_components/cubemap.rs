use std::ffi::{c_void, CString};
use std::path::PathBuf;
use hashbrown::HashSet;
use stb_image::stb_image::bindgen::{stbi_image_free, stbi_load};
use crate::helper_things::environment::path_to_bytes;

/// Represents a cubemap, holding the resource and logic to create and use one
pub struct CubeMap
{
    buffer: u32,
    binding_point: u32,
}

/// Result of uploading textures to a cubemap
#[derive(Debug)]
pub enum CubeMapUploadResult
{
    FailedToLoadImage(Box<PathBuf>),
    Success,
    UnsupportedNumberChannels
}

impl CubeMap
{
    /// Creates a new, uninitialized cubemap
    ///
    /// `binding_point` - the sampler binding point for the cubemap
    pub fn new(binding_point: u32) -> CubeMap
    {
        let mut buffer: u32 = 0;

        unsafe
            {
                gl::CreateTextures(gl::TEXTURE_CUBE_MAP, 1, &mut buffer);
            }

        CubeMap{ buffer, binding_point }
    }

    /// Binds the cube map texture to the Texture Cube Map target
    pub fn bind(&mut self)
    {
        unsafe { gl::BindTexture(gl::TEXTURE_CUBE_MAP, self.buffer); }
    }

    /// Uploads the given textures to the cube map. This is a blocking operation.
    /// There must be 6 textures to load, all of the same format, in the following order:
    ///
    /// 1. Right
    /// 2. Left
    /// 3. Top
    /// 4. Bottom
    /// 5. Front
    /// 6. Back
    ///
    /// `texture_locations` - location of the textures to use for the cube map
    pub fn upload_texture_sequentially(&mut self, texture_locations: Vec<PathBuf>) -> Result<CubeMapUploadResult, CubeMapUploadResult>
    {
        self.bind();
        unsafe{ gl::BindTextureUnit(self.binding_point, self.buffer) }

        let number_sides_cube_map= 6;
        debug_assert!(texture_locations.len() == number_sides_cube_map);

        // Keep track of what texture formats are passed into the cubemap
        let mut uploaded_texture_formats: HashSet<u32> = HashSet::default();

        for (index, texture) in texture_locations.into_iter().enumerate()
        {
            let mut width = 0;
            let mut height = 0;
            let mut nr_channels = 0;

            let image_data = unsafe
                {
                    let texture_cstring = CString::new(path_to_bytes(texture.clone())).unwrap();
                    stbi_load(texture_cstring.as_ptr(), &mut width, &mut height, &mut nr_channels, 0)
                };

            if image_data == std::ptr::null_mut()
            {
                return Err(CubeMapUploadResult::FailedToLoadImage(Box::new(texture)));
            }

            let pixel_format = match nr_channels
            {
                3 => gl::RGB,
                4 => gl::RGBA,
                _ => return Err(CubeMapUploadResult::UnsupportedNumberChannels)
            };

            unsafe
                {
                    gl::TexImage2D(
                        gl::TEXTURE_CUBE_MAP_POSITIVE_X + index as u32,
                        0,
                        pixel_format as i32,
                        width as i32,
                        height as i32,
                        0,
                        pixel_format,
                        gl::UNSIGNED_BYTE,
                        image_data as *const c_void
                    );
                }

            unsafe { stbi_image_free(image_data as *mut c_void) }

            uploaded_texture_formats.insert(pixel_format);
        }

        // All textures must have the same format- otherwise hard to debug OpenGL errors appear
        debug_assert!(uploaded_texture_formats.len() == 1);

        unsafe
            {
                gl::TexParameteri(gl::TEXTURE_CUBE_MAP, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
                gl::TexParameteri(gl::TEXTURE_CUBE_MAP, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
                gl::TexParameteri(gl::TEXTURE_CUBE_MAP, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
                gl::TexParameteri(gl::TEXTURE_CUBE_MAP, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
                gl::TexParameteri(gl::TEXTURE_CUBE_MAP, gl::TEXTURE_WRAP_R, gl::CLAMP_TO_EDGE as i32);
            }

        Ok(CubeMapUploadResult::Success)
    }
}
