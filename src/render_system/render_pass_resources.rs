use std::path::PathBuf;
use hashbrown::HashMap;
use crate::objects::ecs::ECS;
use crate::objects::entity_id::EntityId;
use crate::render_components::cubemap::CubeMap;
use crate::render_components::frame_buffer::FBO;
use crate::render_components::mapped_buffer::MappedBuffer;
use crate::render_components::shader_program::ShaderProgram;
use crate::render_components::texture_array::TextureArray;
use crate::render_components::vao::VAO;
use crate::render_system::initialize_logic::{ExpectedUniformData, UniformDataLocation};
use crate::render_system::render_system::{ModelUpdateFunction, UploadedTextureLocation};

/// Holds the variables required to execute a first or second render pass
pub struct RenderPassResources
{
    pub shader_program: ShaderProgram,
    pub vao: VAO,
    pub vertex_shader_resource: VertexShaderResources,
    pub fragment_shader_resource: FragmentShaderResources,
    pub uniform_resources: UniformResources,
    pub uploaded_textures: HashMap<PathBuf, UploadedTextureLocation>,
    pub shadow_map_binding_point: Option<u32>,
    pub deferred_rendering_fbo: Option<FBO>,
}

/// Holds information about updating vertex layouts
pub struct VertexShaderResources
{
    pub indice_buffer: Option<MappedBuffer>,
    pub per_model_buffers: Vec<MappedBuffer>,
    pub per_instance_buffers: Vec<MappedBuffer>,
    pub layout_update_fn: Option<fn(u32, &ECS, &mut Vec<u8>, EntityId)>,
    pub model_update_fn: ModelUpdateFunction,
    pub model_layout_indexes: Vec<u32>,
    pub instance_layout_indexes: Vec<u32>,
}

/// Holds information about updating textures
pub struct FragmentShaderResources
{
    pub texture_arrays: Vec<TextureArray>,
    pub texture_lookup: HashMap<String, usize>,
    pub cube_maps: HashMap<String, CubeMap>,
}

/// Holds information to locate where to write uniform data into
pub struct UniformResources
{
    pub mapped_buffers: Vec<MappedBuffer>,
    pub uniform_location_map: HashMap<String, UniformDataLocation>,
    pub uniform_type_ids: HashMap<String, ExpectedUniformData>,
    pub uniform_entities: HashMap<String, EntityId>,
    pub ecs: ECS,
}

/// Passed into uniform update function to write updated values for uniforms
pub struct UniformBufferInformation<'a>
{
    pub uniform_location: &'a HashMap<String, UniformDataLocation>,
    pub uniform_type: &'a HashMap<String, ExpectedUniformData>,
    pub buffers: &'a mut Vec<MappedBuffer>,
    pub buffers_to_flush: Vec<usize>,
    pub buffers_to_fence: Vec<usize>,
}