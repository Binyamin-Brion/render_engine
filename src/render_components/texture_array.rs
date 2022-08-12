use std::ffi::{c_void, CString};
use std::mem::size_of;
use std::path::PathBuf;
use std::ptr::copy_nonoverlapping;
use stb_image::stb_image::bindgen::{stbi_image_free, stbi_load, stbi_set_flip_vertically_on_load};
use crate::helper_things::environment::path_to_bytes;
use crate::render_system::system_information::{TextureFormat, TextureInformation};

/// Represents a texture array that can be used to store textures. The array is immutable and holds
/// textures of a specific size.
 // Incorrect warnings from compiler- fields it says aren't read are actually used
pub struct TextureArray
{
    buffers: Vec<u32>,
    texture_array_info: TextureInformation,
    number_textures_held: i32,
    current_buffer_index: usize,
    binding_point: u32,
}

/// Possible result of uploading a texture. This enum contains both success and error values;
/// return result uses Option to differentiate between success and failure.
#[derive(Debug)]
 // Incorrect warnings from compiler- all variants are used when uploading texture
pub enum TextureUploadResult
{
    FailedToLoadFile(String),
    UnsupportedNumberChannels,
    TextureArrayFull,
    Success(i32),
    SuccessWithResize(i32, f32, f32),
}

/// Specifies characteristics about a texture to upload
pub struct TextureProperties
{
    pub width: i32,
    pub height: i32,
    pub nr_channels: i32,
    image_data: *mut u8,
}

impl TextureArray
{
    /// Create a new texture array with the given parameters
    ///
    /// `texture_array_info` - the information specifying information about the texture array to create
    /// `number_buffers` - the number of round-robin buffers to use for the texture array
    /// `binding_point` - the sampler binding point that this texture array should bind to
    pub fn new(texture_array_info: TextureInformation, number_buffers: usize, binding_point: u32) -> TextureArray
    {
        let mut buffers = Vec::with_capacity(number_buffers);

        unsafe
            {
                for _ in 0..number_buffers
                {
                    let mut new_buffer: u32 = 0;

                    gl::CreateTextures(gl::TEXTURE_2D_ARRAY, 1, &mut new_buffer);
                    // Direct state access is used; no need to mark texture unit as active
                    gl::TextureStorage3D(new_buffer, texture_array_info.number_mipmaps, texture_array_info.format as gl::types::GLenum, texture_array_info.width, texture_array_info.height, texture_array_info.number_textures);

                    // Not to sure why the gl enums need to be casted to an i32 when the OpenGL enum is u32...
                    gl::TextureParameteri(new_buffer, gl::TEXTURE_MIN_FILTER, texture_array_info.min_filter_options as i32);
                    gl::TextureParameteri(new_buffer, gl::TEXTURE_MAG_FILTER, texture_array_info.mag_filter_options as i32);
                    gl::TextureParameteri(new_buffer, gl::TEXTURE_WRAP_S, texture_array_info.wrap_s as i32 as i32);
                    gl::TextureParameteri(new_buffer, gl::TEXTURE_WRAP_T, texture_array_info.wrap_t as i32);

                    if let Some(border_colour) = texture_array_info.border_color
                    {
                        gl::TextureParameterfv(new_buffer, gl::TEXTURE_BORDER_COLOR, border_colour.as_ptr());
                    }

                    buffers.push(new_buffer);
                }
            }

        TextureArray{ buffers, texture_array_info, number_textures_held: 0, current_buffer_index: 0, binding_point }
    }

    /// Adds a texture that is a single colour to a layer of the texture array
    ///
    /// `colour` - the colour the texture layer should have
    pub fn add_texture_solid_colour(&mut self, colour: [u8; 4]) -> i32
    {
        if self.number_textures_held == self.texture_array_info.number_textures
        {
            // TODO: Should this be a panic or default to not enough storage for texture colour, like
            // TODO: when calling add_texture_sequentially_from_file_stbi?
            eprintln!("Not enough storage when adding solid colour texture. Max amount: {}", self.texture_array_info.number_textures);
        }

        let pixels_required = self.texture_array_info.width * self.texture_array_info.height;
        let pixel_data = vec![colour; pixels_required as usize];

        unsafe
            {
                gl::TextureSubImage3D(self.buffers[self.current_buffer_index],
                                      0,
                                      0, 0, self.number_textures_held,
                                      self.texture_array_info.width, self.texture_array_info.height, 1,
                                      gl::RGBA, gl::UNSIGNED_BYTE, pixel_data.as_ptr() as *const c_void);
            }

        self.number_textures_held += 1;
        self.number_textures_held - 1
    }

    /// Adds a texture to the array, blocking the calling thread.
    ///
    /// The texture to upload must be smaller than the textures held within the array, and the array must not be full.
    ///
    /// If the texture is smaller, then the texture is resized to the correct size to be stored in the texture array.
    /// The resize is done by creating a texture of the appropriate size and overlaying the given texture on-top of it.
    /// The resulting upload result will contain values describing how much the texture's UV coordinates need to
    /// be modified as a result of the texture resize. These values will be [0, 1) - multiply these values with the
    /// texture coordinates U and V coordinates respectfully
    ///
    /// `texture_properties` - the properties of the texture to upload

    pub fn add_texture_sequentially_from_file_stbi(&mut self, texture_properties: &TextureProperties) -> Result<TextureUploadResult, TextureUploadResult>
    {
        if self.number_textures_held == self.texture_array_info.number_textures
        {
            return Err(TextureUploadResult::TextureArrayFull);
        }

        let pixel_format = match texture_properties.nr_channels
        {
            3 => gl::RGB,
            4 => gl::RGBA,
            _ => return Err(TextureUploadResult::UnsupportedNumberChannels)
        };

        if texture_properties.width < self.texture_array_info.width || texture_properties.height < self.texture_array_info.height
        {
            let bytes_required =  self.texture_array_info.width * self.texture_array_info.height;

            let bytes_required = bytes_required * texture_properties.nr_channels;
            let mut pixels = vec![0; bytes_required as usize];

            // Overlay loaded image onto a texture of the correct size
            for x in 0..texture_properties.height
            {
                let size_pixel = (texture_properties.nr_channels * size_of::<u8>() as i32) as isize;

                let source_offset = (x * texture_properties.width) as isize * size_pixel;
                let destination_offset = (self.texture_array_info.width * x) as isize * size_pixel;
                let number_bytes_to_copy = (texture_properties.width as isize * size_pixel) as usize;

                unsafe{ copy_nonoverlapping(texture_properties.image_data.offset(source_offset), pixels.as_mut_ptr().offset(destination_offset), number_bytes_to_copy) }
            }

            unsafe
                {
                    gl::TextureSubImage3D(self.buffers[self.current_buffer_index],
                                          0,
                                          0, 0, self.number_textures_held,
                                          self.texture_array_info.width, self.texture_array_info.height, 1,
                                          pixel_format, gl::UNSIGNED_BYTE, pixels.as_ptr() as *const c_void);
                }

            let resize_factor_width = texture_properties.width as f32 / self.texture_array_info.width as f32;
            let resize_factor_height = texture_properties.height as f32 / self.texture_array_info.height as f32;

            self.number_textures_held += 1;

            return Ok(TextureUploadResult::SuccessWithResize(self.number_textures_held - 1, resize_factor_width, resize_factor_height));
        }
        else
        {
            self.number_textures_held += 1;

            unsafe
                {
                    gl::TextureSubImage3D(self.buffers[self.current_buffer_index],
                                          0,
                                          0, 0, self.number_textures_held,
                                          self.texture_array_info.width, self.texture_array_info.height, 1,
                                          pixel_format, gl::UNSIGNED_BYTE, texture_properties.image_data as *const c_void);
                }
        }

        Ok(TextureUploadResult::Success(self.number_textures_held - 1))
    }

    /// Binds the texture array to the texture unit specified in the array constructor
    pub fn bind_texture_to_texture_unit(&mut self)
    {
        unsafe
            {
                gl::BindTextureUnit(self.binding_point, self.buffers[self.current_buffer_index])
            }
    }

    /// Binds the texture array to the sampler binding point provided
    ///
    /// `binding_point` - the sampler binding point to bind to
    pub fn bind_to_specific_texture_unit(&mut self, binding_point: u32)
    {
        unsafe
            {
                gl::BindTextureUnit(binding_point, self.buffers[self.current_buffer_index])
            }
    }

    /// Create mipmaps for the texture array. This should only be called once all textures have been
    /// uploaded into the array

    pub fn create_mipmaps(&self)
    {
        unsafe
            {
                gl::GenerateMipmap(gl::TEXTURE_2D_ARRAY);
            }
    }

    /// Find how much space (vRam) would be wasted if a texture with the given properties were to
    /// be uploaded to this texture array. An Err result indicate that the given texture cannot be
    /// uploaded to this texture
    ///
    /// `texture_properties` - the properties of the texture to upload to
    pub fn query_wasted_space(&self, texture_properties: &TextureProperties) -> Result<usize, ()>
    {
        if self.number_textures_held == self.texture_array_info.number_textures
        {
            return Err(());
        }

        if self.texture_array_info.format == TextureFormat::RGB && texture_properties.nr_channels == 4 // Requires RGBA
        {
            return Err(());
        }

        if self.texture_array_info.width < texture_properties.width || self.texture_array_info.height < texture_properties.height
        {
            return Err(());
        }

        let wasted_width = self.texture_array_info.width - texture_properties.width;
        let wasted_height = self.texture_array_info.height - texture_properties.height;
        let multiplier = if self.texture_array_info.format == TextureFormat::RGBA && texture_properties.nr_channels == 3
        {
            32.0 / 24.0
        }
        else
        {
            1.0
        };

        let number_wasted_pixels = (wasted_width * wasted_height) as f32;
        let adjusted_wasted_pixels = (number_wasted_pixels * multiplier).ceil() as usize;

        Ok(adjusted_wasted_pixels)
    }

    /// Get the raw OpenGL resource for this texture array
    pub fn get_raw_resource(&self) -> u32
    {
        self.buffers[self.current_buffer_index]
    }
}

impl TextureProperties
{
    /// Read an image and query its properties
    ///
    /// `texture_location` - the location of the texture to read
    pub fn read_image(texture_location: &PathBuf) -> TextureProperties
    {
        let mut width = 0;
        let mut height = 0;
        let mut nr_channels = 0;

        let image_data = unsafe
            {
                stbi_set_flip_vertically_on_load(1 as i32);
                let image_location_cstring = CString::new(path_to_bytes(texture_location.clone())).unwrap();
                stbi_load(image_location_cstring.as_ptr(), &mut width, &mut height, &mut nr_channels, 0)
            };

        if image_data.is_null()
        {
            panic!("Failed to read the texture: {:?}", texture_location);
        }

        TextureProperties { width, height, nr_channels, image_data }
    }
}

impl Drop for TextureProperties
{
    fn drop(&mut self)
    {
        unsafe{ stbi_image_free(self.image_data as *mut c_void) }
    }
}