use std::env;
use crate::render_components::texture_array::TextureArray;
use crate::render_system::system_information::TextureInformation;

const MIN_NUMBER_COLOUR_ATTACHMENTS: usize = 8;

/// Abstraction over a frame buffer object, providing logic to create and use a FBO
#[allow(dead_code)]
pub struct FBO
{
    fbo: u32,
    colour_texture: [Option<TextureArray>; MIN_NUMBER_COLOUR_ATTACHMENTS],
    depth_texture: Option<TextureArray>,
    depth_stencil_texture: Option<TextureArray>,
    stencil_texture: Option<TextureArray>,
    no_colour_attachments: bool,
}

/// Possible formats of attachments to the FBO
#[derive(Copy, Clone)]
#[repr(u32)]
pub enum AttachmentFormat
{
    RGB = gl::RGB,
    DepthAttachment = gl::DEPTH_COMPONENT,
    StencilAttachment = gl::STENCIL_INDEX,
    DepthAndStencilAttachment = gl::DEPTH24_STENCIL8,
}

/// Targets that the FBO can be bound to
#[repr(u32)]
#[allow(dead_code)]
pub enum BindingTarget
{
    DrawFrameBuffer = gl::DRAW_FRAMEBUFFER,
    ReadFrameBuffer = gl::READ_FRAMEBUFFER,
}

impl FBO
{
    /// Create a new FBO that has at the minimum, the required attachments for the FBO to be valid
    ///
    /// `colour_attachment` - the colour attachments of the FBO
    /// `depth_attachment` - the optional depth attachment
    /// `stencil_attachment` - the optional stencil attachment
    /// `depth_stencil_attachment` - the optional depth-stencil attachment
    pub fn new(colour_attachment: Vec<TextureInformation>, depth_attachment: Option<TextureInformation>,
               stencil_attachment: Option<TextureInformation>, depth_stencil_attachment: Option<TextureInformation>) -> Result<FBO, String>
    {
        let mut fbo: u32 = 0;
        let mut colour_texture = [None, None, None, None, None, None, None, None,];
        let mut depth_texture = None;
        let mut depth_stencil_texture = None;
        let mut stencil_texture = None;
        unsafe
            {
                gl::CreateFramebuffers(1, &mut fbo);
            }

        let mut colour_attachments = vec![];
        for (index, x) in colour_attachment.into_iter().enumerate()
        {
            FBO::setup_attachment_internal(x, AttachmentFormat::RGB,  Some(index as u32), fbo, &mut colour_texture[index]);
            colour_attachments.push(gl::COLOR_ATTACHMENT0 + index as u32);
        }

        if let Some(depth_attachment) = depth_attachment
        {
            FBO::setup_attachment_internal(depth_attachment, AttachmentFormat::DepthAttachment, None, fbo, &mut depth_texture);
        }

        if let Some(stencil_attachment) = stencil_attachment
        {
            FBO::setup_attachment_internal(stencil_attachment, AttachmentFormat::StencilAttachment,  None, fbo, &mut stencil_texture);
        }

        if let Some(depth_stencil_attachment) = depth_stencil_attachment
        {
            FBO::setup_attachment_internal(depth_stencil_attachment, AttachmentFormat::DepthAndStencilAttachment, None, fbo, &mut depth_stencil_texture);
        }

        // For some reason, checking FBO status causes render doc to close program unexpectedly.
        // When using render doc as as result, just assume valid FBO was created
        let creation_code = match env::var("using_render_doc")
        {
            Ok(_) => None,
            Err(_) => Some(unsafe{ gl::CheckNamedFramebufferStatus(fbo, gl::FRAMEBUFFER) })
        };

        match creation_code
        {
            None =>
                {
                    unsafe{ gl::NamedFramebufferDrawBuffers(fbo, colour_attachments.len() as i32, colour_attachments.as_ptr()) }
                    Ok(FBO{ fbo, colour_texture, depth_texture, depth_stencil_texture, stencil_texture, no_colour_attachments: colour_attachments.is_empty() })
                },
            Some(i) =>
                {
                    if i == gl::FRAMEBUFFER_COMPLETE
                    {
                        unsafe{ gl::NamedFramebufferDrawBuffers(fbo, colour_attachments.len() as i32, colour_attachments.as_ptr()) }
                        Ok(FBO{ fbo, colour_texture, depth_texture, depth_stencil_texture, stencil_texture, no_colour_attachments: colour_attachments.is_empty() })
                    }
                    else
                    {
                        match i
                        {
                            gl::FRAMEBUFFER_UNDEFINED => Err("FBO creation code: FRAMEBUFFER_UNDEFINED".to_string()),
                            gl::FRAMEBUFFER_INCOMPLETE_ATTACHMENT => Err("FBO creation code: FRAMEBUFFER_INCOMPLETE_ATTACHMENT".to_string()),
                            gl::FRAMEBUFFER_INCOMPLETE_MISSING_ATTACHMENT => Err("FBO creation code: FRAMEBUFFER_INCOMPLETE_MISSING_ATTACHMENT".to_string()),
                            gl::FRAMEBUFFER_INCOMPLETE_DRAW_BUFFER => Err("FBO creation code: FRAMEBUFFER_INCOMPLETE_DRAW_BUFFER".to_string()),
                            gl::FRAMEBUFFER_INCOMPLETE_READ_BUFFER => Err("FBO creation code: FRAMEBUFFER_INCOMPLETE_READ_BUFFER".to_string()),
                            gl::FRAMEBUFFER_UNSUPPORTED => Err("FBO creation code: FRAMEBUFFER_UNSUPPORTED".to_string()),
                            gl::FRAMEBUFFER_INCOMPLETE_MULTISAMPLE => Err("FBO creation code: FRAMEBUFFER_INCOMPLETE_MULTISAMPLE".to_string()),
                            gl::FRAMEBUFFER_INCOMPLETE_LAYER_TARGETS => Err("FBO creation code: FRAMEBUFFER_INCOMPLETE_LAYER_TARGETS".to_string()),
                            gl::INVALID_ENUM => Err("FBO creation code: INVALID_ENUM".to_string()),
                            gl::INVALID_OPERATION => Err("FBO creation code: INVALID_OPERATION".to_string()),
                            _ => Err(format!("Unknown FBO creation code of: {}", i))
                        }
                    }
                }
        }
    }

    /// Binds the FBO, making subsequent render operations affect this FBO
    ///
    /// `bind_target` - the target to bind the FBO to
    pub fn bind_fbo(&mut self, bind_target: BindingTarget)
    {
        unsafe
            {
                gl::BindFramebuffer(bind_target as u32, self.fbo);

                if self.no_colour_attachments
                {
                    gl::DrawBuffer(gl::NONE);
                    gl::ReadBuffer(gl::NONE);
                }
            }
    }

    /// Bind the texture holding the depth information to the sampler binding point given
    ///
    /// `binding_point` - the binding point to bind the depth attachment texture to
    pub fn bind_depth_texture_to_specific_texture_unit(&mut self, binding_point: u32)
    {
        if let Some(ref mut depth_texture) = self.depth_texture
        {
            depth_texture.bind_to_specific_texture_unit(binding_point);
        }
    }

    /// Binds the colour attachment textures to the sampler bindings points
    ///
    /// `binding_points` - the sampler binding points to bind each colour attachment to
    pub fn bind_colour_textures(&mut self, binding_points: Vec<u32>)
    {
        for (index, x) in self.colour_texture.iter_mut().filter_map(|x| x.as_mut()).enumerate()
        {
            x.bind_to_specific_texture_unit(binding_points[index]);
        }
    }

    /// Marks a specific layer within a texture layer used as an attachment as the storage for
    /// rendering operations
    ///
    /// `format` - the type of rendering operation result that should be stored in the texture layer
    ///             referenced by this function call
    /// `texture_array_index` - the layer of the texture array to use for storage
    pub fn setup_attachment(&mut self, format: AttachmentFormat, texture_array_index: i32)
    {
        unsafe
            {
                match format
                {
                    AttachmentFormat::DepthAttachment =>
                        {
                            let texture_array = self.depth_texture.as_ref().unwrap();
                            gl::NamedFramebufferTextureLayer(self.fbo, gl::DEPTH_ATTACHMENT, texture_array.get_raw_resource(), 0, texture_array_index)
                        },
                    AttachmentFormat::DepthAndStencilAttachment =>
                        {
                            let texture_array = self.stencil_texture.as_ref().unwrap();
                            gl::NamedFramebufferTextureLayer(self.fbo, gl::DEPTH_STENCIL_ATTACHMENT, texture_array.get_raw_resource(), 0, 0)
                        },
                    _ => {}
                }
            }
    }

    /// Marks a specific layer within a texture layer used as an attachment as the storage for
    /// rendering operations. This is used only for when the FBO is created, and can be used for all of
    /// the attachments of the FBO
    ///
    /// `texture_array_info` - the information required to create a texture array for the FBO attachment
    /// `format` - the attachment that is being setup
    /// `colour_index` - if a colour attachment is being setup, the index of the colour attachment, starting
    ///                 at index 0
    /// `fbo` - the raw FBO resource to will have an attachment attached to it
    /// `handler` - variable that will own the created texture array for the attachment
    fn setup_attachment_internal(texture_array_info: TextureInformation, format: AttachmentFormat, colour_index: Option<u32>, fbo: u32, handler: &mut Option<TextureArray>)
    {
        let texture_array = TextureArray::new(texture_array_info, 1, 0);

        unsafe
            {
                match format
                {
                    AttachmentFormat::RGB =>
                        {
                            let attachment_index = colour_index.unwrap_or(0);
                            gl::NamedFramebufferTextureLayer(fbo, gl::COLOR_ATTACHMENT0 + attachment_index, texture_array.get_raw_resource(), 0, 0)
                        },
                    AttachmentFormat::DepthAttachment => gl::NamedFramebufferTextureLayer(fbo, gl::DEPTH_ATTACHMENT, texture_array.get_raw_resource(), 0, 0),
                    AttachmentFormat::DepthAndStencilAttachment => gl::NamedFramebufferTextureLayer(fbo, gl::DEPTH_STENCIL_ATTACHMENT, texture_array.get_raw_resource(), 0, 0),
                    AttachmentFormat::StencilAttachment => gl::NamedFramebufferTextureLayer(fbo, gl::STENCIL_ATTACHMENT, texture_array.get_raw_resource(), 0, 0)
                }
            }

        *handler = Some(texture_array);
    }
}

