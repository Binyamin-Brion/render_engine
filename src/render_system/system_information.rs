use std::path::PathBuf;
use hashbrown::{HashMap, HashSet};
use nalgebra_glm::{TMat4, TVec3, TVec4};
use serde::{Serialize, Deserialize};
use crate::exports::camera_object::Camera;
use crate::exports::load_models::MaxNumLights;
use crate::exports::rendering::{DrawParam, LevelOfView};
use crate::objects::ecs::ECS;
use crate::objects::entity_id::EntityId;
use crate::render_components::frame_buffer::FBO;
use crate::render_system::render_system::ModelUpdateFunction;
use crate::window::input_state::InputHistory;
use crate::world::bounding_box_tree_v2::{BoundingBoxTree, UniqueWorldSectionId};

/// ******** Vertex Shader Options *************

/// >>>>>>>>>>>> Enums <<<<<<<<<<<<<<

/// Possible GLSL layout types. The data type is specified in each enum
/// as the data structure of the layout does not specify this. For example,
/// an ivec3 could three ints, or three shorts that are interpreted as ints
#[derive(Copy, Clone)]
#[allow(dead_code)]
pub enum LayoutType
{
    Vec2Int,
    Vec2Uint,
    Vec2Float,
    Vec3Float,
    Vec4Float,
    Vec3Uint,
    Vec4Uint,
    Mat4x4Float
}

impl LayoutType
{
    /// Converts the layout type to its string representation
    pub fn to_string(&self) -> String
    {
        match *self
        {
            LayoutType::Vec2Int => "ivec2".to_string(),
            LayoutType::Vec2Uint => "uvec2".to_string(),
            LayoutType::Vec2Float => "vec2".to_string(),
            LayoutType::Vec3Float => "vec3".to_string(),
            LayoutType::Vec4Float => "vec4".to_string(),
            LayoutType::Vec3Uint => "uvec3".to_string(),
            LayoutType::Vec4Uint => "uvec4".to_string(),
            LayoutType::Mat4x4Float => "mat4x4".to_string()
        }
    }
}

pub type NumberBuffers = usize;
pub type SizeBufferBytes = isize;

/// Specifies the buffer division index for a layout.
/// The number of buffers specify how many buffers will be used to
/// avoid stalling the pipeline when the contents of a layout need to be updated
#[derive(Copy, Clone)]
pub enum LayoutInstance
{
    Divisor0(NumberBuffers, SizeBufferBytes),
    Divisor1(NumberBuffers, SizeBufferBytes)
}

/// Specifies if a layout is to be used for model information or for instancing information
pub enum LayoutUse
{
    PerModel,
    PerInstance,
}

/// >>>>>>>>>> Structures <<<<<<<<<<<<<

/// Specifies the information required to create a Mapped Buffer for indices
#[derive(Copy, Clone)]
pub struct IndiceInformation
{
    pub number_buffers: usize,
    pub buffer_size_bytes: isize,
}

impl IndiceInformation
{
    /// Specifies the information needed to create a mapped buffer for indices
    pub fn new(number_buffers: usize, buffer_size_bytes: isize) -> IndiceInformation
    {
        IndiceInformation{number_buffers, buffer_size_bytes}
    }
}

/// The information required to specify a layout. The shader that a
/// layout is a part of is implicitly is done as a part of the system builder.
pub struct LayoutInformation
{
    pub data_type: LayoutType,
    pub instance: LayoutInstance,
    pub layout_use: LayoutUse,
    pub name: String,
}

impl LayoutInformation
{
    /// Specifies the information to create a mapped buffer for a vertex layout input
    pub fn new<A: Into<String>>(data_type: LayoutType, instance: LayoutInstance, layout_use: LayoutUse, name: A) -> LayoutInformation
    {
        LayoutInformation{ data_type, instance, layout_use, name: name.into() }
    }
}

/// Specifies layout information for the fragment shader, and the parameters for the texture
/// that the layout will write to
pub struct FragLayoutInformation
{
    pub data_type: LayoutType,
    pub backing_texture_format: TextureFormat,
    pub initial_fbo_size: (i32, i32),
    pub name: String,
}

impl FragLayoutInformation
{
    /// Creates a new FragLayoutInformation instance
    ///
    /// `data_type` - the type of data the layout will be writing
    /// `texture_format` - the type of texture the layout will write to
    /// `window_resolution` - the dimensions of the texture the layout will write to
    /// `name` - the name of the texture the layout will write to
    pub fn new<A: Into<String>>(data_type: LayoutType, texture_format: TextureFormat, window_resolution: (i32, i32), name: A) -> FragLayoutInformation
    {
        FragLayoutInformation{ data_type, name: name.into(), initial_fbo_size: window_resolution, backing_texture_format: texture_format }
    }
}

/// Information to declare GLSL version
pub enum GLSLVersion
{
    Core430
}

impl GLSLVersion
{
    /// Convert the enum to its string representation
    pub fn to_string(&self) -> String
    {
        match *self
        {
            GLSLVersion::Core430 => "#version 430 core".to_string()
        }
    }
}

/// Information to declare constants
pub enum ConstantValue
{
    UInt(u32),
}

impl ConstantValue
{
    /// Convert the enum to its string representation
    pub fn to_string(&self) -> (String, String)
    {
        match *self
        {
            ConstantValue::UInt(i) => ("uint".to_string(), i.to_string())
        }
    }
}

/// Specifies the location of a constant (which shader is it in)
pub enum ConstantLocation
{
    VertexShader,
    FragmentShader,
}

/// Information to use a constant in a shader
pub struct Constant
{
    pub value: ConstantValue,
    pub name: String,
    pub share_targets: Vec<ConstantLocation>,
}

impl Constant
{
    /// Create a new shader consta nt
    ///
    /// `value` - the value of the constant
    /// `name` - the name by which the constant should be referred to by
    /// `targets` - the locations the constant will be available
    pub fn new<A: Into<String>>(value: ConstantValue, name: A, targets: Vec<ConstantLocation>) -> Constant
    {
        Constant{ value, name: name.into(), share_targets: targets }
    }
}

/// Information required to declare out variables
#[derive(Copy, Clone)]
#[allow(dead_code)]
pub enum SharedVariableType
{
    Vec2,
    Vec3,
    Vec4,
    Mat4x4Float,
    Int,
    UInt,
    Float,
    UVec3,
    UVec4,
    UIntArray(u16),
    FloatArray(u16),
    Vec3Array(u16),
    Vec4Array(u16),
    Mat4Array(u16),
}

impl SharedVariableType
{
    /// Convert the enum to its string representation
    pub fn to_string(&self) -> String
    {
        match *self
        {
            SharedVariableType::UVec3 => "uvec3".to_string(),
            SharedVariableType::UVec4 => "uvec4".to_string(),
            SharedVariableType::Vec2 =>"vec2".to_string(),
            SharedVariableType::Vec3 => "vec3".to_string(),
            SharedVariableType::Vec4 => "vec4".to_string(),
            SharedVariableType::Mat4x4Float => "mat4".to_string(),
            SharedVariableType::Int => "int".to_string(),
            SharedVariableType::UInt => "uint".to_string(),
            SharedVariableType::Float => "float".to_string(),
            SharedVariableType::UIntArray(_) => "uint".to_string(),
            SharedVariableType::FloatArray(_) => "float".to_string(),
            SharedVariableType::Vec3Array(_) => "vec3".to_string(),
            SharedVariableType::Vec4Array(_) => "vec4".to_string(),
            SharedVariableType::Mat4Array(_) => "mat4".to_string(),
        }
    }
}

/// Specifies the location of where the out variable are going
pub enum SharedTarget
{
    FragmentShader
}

/// Information to required to create out variables
pub struct OutVariables
{
    pub data_type: SharedVariableType,
    pub name: String,
    pub is_flat: bool,
    pub share_target: Vec<SharedTarget>,
}

impl OutVariables
{
    /// Specify the creation of a new out variable
    ///
    /// `data_type` - the type the out variable is
    /// `name` - the name of the out variable
    /// `is_flat` - boolean determining if the out variable will be flat
    /// `share_target` - location that the variable will be outputted to
    pub fn new<A: Into<String>>(data_type: SharedVariableType, name: A, is_flat: bool, share_target: Vec<SharedTarget>,) -> OutVariables
    {
        OutVariables{ data_type, name: name.into(), is_flat, share_target }
    }
}

/// ************ Fragment Shader Options ****************

/// >>>>>>>>>>> Enums <<<<<<<<<<<<<

/// Specifies the format that a texture can have
#[repr(u32)]
#[derive(Copy, Clone, Eq, PartialEq)]
#[allow(dead_code)]
pub enum TextureFormat
{
    Depth = gl::DEPTH_COMPONENT24,
    DepthStencil = gl::DEPTH24_STENCIL8,
    RGB = gl::RGB8,
    RGBA = gl::RGBA8,
    SRGBA = gl::SRGB8_ALPHA8,
    RGBA16F = gl::RGBA32F,
    RG8 = gl::RG8,
}

/// Specifies required information to allocate a texture array
#[derive(Clone)]
pub struct TextureInformation
{
    pub sampler_name: String,
    pub number_mipmaps: i32,
    pub format: TextureFormat,
    pub min_filter_options: MinFilterOptions,
    pub mag_filter_options: MagFilterOptions,
    pub wrap_s: TextureWrap,
    pub wrap_t: TextureWrap,
    pub width: i32,
    pub height: i32,
    pub number_textures: i32,
    pub border_color: Option<TVec4<f32>>
}

/// Type of min filter option to use for textures
#[repr(u32)]
#[derive(Copy, Clone)]
#[allow(dead_code)]
pub enum MinFilterOptions
{
    Nearest = gl::NEAREST,
    Linear = gl::LINEAR,
    NearestMipMapNearest = gl::NEAREST_MIPMAP_NEAREST,
    LinearMipMapNearest = gl::LINEAR_MIPMAP_NEAREST,
    NearestMipMapLinear = gl::NEAREST_MIPMAP_LINEAR,
    LinearMipMapLinear = gl::LINEAR_MIPMAP_LINEAR,
}

/// Type of mag filter option to use for textures
#[repr(u32)]
#[derive(Copy, Clone)]
pub enum MagFilterOptions
{
    Nearest = gl::NEAREST,
    Linear = gl::LINEAR,
}

/// Type of texture wrap option to use for textures
#[repr(u32)]
#[derive(Copy, Clone)]
#[allow(dead_code)]
pub enum TextureWrap
{
    ClampToEdge = gl::CLAMP_TO_EDGE,
    ClampToBorder = gl::CLAMP_TO_BORDER,
    MirroredRepeat = gl::MIRRORED_REPEAT,
    Repeat = gl::REPEAT,
    MirrorClampToEdge = gl::MIRROR_CLAMP_TO_EDGE,
}

/// >>>>>>>>>>>> Structures <<<<<<<<<<<<<<<

/// Specifies required information to allocate a cube map
#[derive(Clone)]
pub struct CubeMapInitInfo
{
    pub cube_map_name: String,
}

impl CubeMapInitInfo
{
    /// Specifies the information required to create a cubemap
    ///
    /// `cubemap_name` - the name of the cubemap to create
    pub fn new<T: Into<String>>(cubemap_name: T) -> CubeMapInitInfo
    {
        CubeMapInitInfo{ cube_map_name: cubemap_name.into() }
    }
}

/// ************ Uniform Options *************

/// >>>>>>>>>>> Enums <<<<<<<<<<<<<

/// Specifies the binding point for a uniform. All uniforms must be in a binding
/// point; no global uniforms are allowed
#[repr(u32)]
#[derive(Copy, Clone, Hash, Eq, PartialEq)]
#[allow(dead_code)]
pub enum UniformBinding
{
    Point0 = 0,
    Point1 = 1,
}

/// Specifies the type of uniform data. Specific data type must be specified
/// as certain data types do not specify type of member variables. For example,
/// ivec3 can be three ints or three shorts
#[derive(Copy, Clone)]
#[allow(dead_code)]
pub enum UniformType
{
    Vec3,
    Vec4,
    Mat4x4Float,
    Int,
    UInt,
    Float,
    UIntArray(u16),
    FloatArray(u16),
    Vec3Array(u16),
    Vec4Array(u16),
    Mat4Array(u16),
}

impl UniformType
{
    /// Convert the enum to its string representation
    pub fn to_string(&self) -> String
    {
        match *self
        {
            UniformType::Vec3 => "vec3".to_string(),
            UniformType::Vec4 => "vec4".to_string(),
            UniformType::Mat4x4Float => "mat4".to_string(),
            UniformType::Int => "int".to_string(),
            UniformType::UInt => "uint".to_string(),
            UniformType::Float => "float".to_string(),
            UniformType::UIntArray(_) => "uint".to_string(),
            UniformType::FloatArray(_) => "float".to_string(),
            UniformType::Vec3Array(_) => "vec3".to_string(),
            UniformType::Vec4Array(_) => "vec4".to_string(),
            UniformType::Mat4Array(_) => "mat4".to_string(),
        }
    }
}

/// >>>>>>>>>>> Structures <<<<<<<<<<<<<

// These structures represent types of uniform available

#[derive(Serialize, Deserialize)]
pub struct UniformVec3(pub TVec3<f32>);

#[derive(Serialize, Deserialize)]
pub struct UniformVec4(pub TVec4<f32>);

#[derive(Serialize, Deserialize)]
pub struct UniformMat4(pub TMat4<f32>);

#[derive(Serialize, Deserialize)]
pub struct UniformInt(pub i32);

#[derive(Serialize, Deserialize)]
pub struct UniformUint(pub u32);

#[derive(Serialize, Deserialize)]
pub struct UniformFloat(pub f32);

#[derive(Serialize, Deserialize)]
pub struct UniformVec3Array(pub Vec<TVec3<f32>>);

#[derive(Serialize, Deserialize)]
pub struct UniformVec4Array(pub Vec<TVec4<f32>>);

#[derive(Serialize, Deserialize)]
pub struct UniformFloatArray(pub Vec<f32>);

#[derive(Serialize, Deserialize)]
pub struct UniformMat4Array(pub Vec<TMat4<f32>>);

#[derive(Serialize, Deserialize)]
pub struct UniformUIntArray(pub Vec<u32>);

/// The information required to specify the contents of a uniform within a shader
#[derive(Clone)]
pub struct UniformBlock
{
    pub block_name: String,
    pub number_buffers: NumberBuffers,
    pub uniforms: Vec<Uniform>,
}

/// Specifies required information to allocate space for a uniform
#[derive(Clone)]
pub struct Uniform
{
    pub name: String,
    pub uniform_type: UniformType,
}

impl UniformBlock
{
    /// Specifies the creation of a new uniform block that contains the given uniforms
    ///
    /// `uniform_block_name` - name of the block containing uniforms
    /// `number_buffers` - number of backing buffers for the block to reduce stalling when updating uniforms
    /// `uniforms` - the uniforms that will be stored in the block
    pub fn new<T: Into<String>>(uniform_block_name: T, number_buffers: NumberBuffers, uniforms: Vec<Uniform>) -> UniformBlock
    {
        UniformBlock{ block_name: uniform_block_name.into(), number_buffers, uniforms }
    }
}

impl Uniform
{
    /// Specifies the information needed to create a uniform
    ///
    /// `name` - the name of the uniform
    /// `uniform_type` - what data will the uniform hold
    pub fn new<T: Into<String>>(name: T, uniform_type: UniformType) -> Uniform
    {
        Uniform { name: name.into(), uniform_type }
    }

    /// Helper function to determine how large a uniform is based off of its type
    ///
    /// `uniform_type` - the type of uniform to get the size for
    pub fn size_uniform_bytes(uniform_type: UniformType) -> usize
    {
        return match uniform_type
        {
            UniformType::Mat4x4Float => 64,
            UniformType::UInt | UniformType::Int | UniformType::Float => 4,
            UniformType::FloatArray(i) => i as usize * 16,
            UniformType::Vec3 => 12,
            UniformType::Vec4 => 16,
            UniformType::UIntArray(i) => i as usize * 16,
            UniformType::Vec3Array(i) => i as usize * 16,
            UniformType::Vec4Array(i) => i as usize * 16,
            UniformType::Mat4Array(i) => i as usize * 64
        }
    }
}

/// *********** Builder Structures **********

/// Information to specify vertex shader and update logic
pub struct VertexShaderInformation
{
    pub write_generated_shader: Option<String>,
    pub glsl_version: GLSLVersion,
    pub shader_source: PathBuf,
    pub layout_info: Vec<LayoutInformation>,
    pub uniforms: Vec<UniformBlock>,
    pub instance_layout_update_fn: Option<fn(u32, &ECS, &mut Vec<u8>, EntityId)>,
    pub model_layout_update_fn: ModelUpdateFunction,
    pub indice_buffers: Option<IndiceInformation>,
    pub out_variables: Vec<OutVariables>,
    pub textures: Vec<TextureInformation>,
    pub cubemaps: Vec<CubeMapInitInfo>,
}

/// Information to specify fragment shader and update logic
pub struct FragmentShaderInformation
{
    pub layouts: Vec<FragLayoutInformation>,
    pub write_generated_shader: Option<String>,
    pub include_error_textures: bool,
    pub include_shadow_maps: bool,
    pub glsl_version: GLSLVersion,
    pub shader_source: PathBuf,
    pub uniforms: Vec<UniformBlock>,
    pub textures: Vec<TextureInformation>,
    pub cubemaps: Vec<CubeMapInitInfo>,
    pub out_variables: Vec<OutVariables>,
}

type EntityLookup = HashMap<String, EntityId>;

/// Holds variables required to prepare information / rendering context before calling the
/// user-defined draw function
pub struct DrawPreparationParameters<'a>
{
    pub visible_sections_light: &'a HashSet<UniqueWorldSectionId>,
    pub shadow_fbo: &'a mut FBO,
    pub logical_entity_lookup: &'a EntityLookup,
    pub logical_ecs: &'a ECS,
    pub camera: &'a Camera,
    pub input_history: &'a InputHistory,
    pub tree: &'a BoundingBoxTree,

    // Lights
    pub visible_directional_lights: &'a mut HashSet::<EntityId>,
    pub visible_point_lights: &'a mut HashSet::<EntityId>,
    pub visible_spot_lights: &'a mut HashSet::<EntityId>,
    pub upload_matrices: &'a Vec<TMat4<f32>>,
    pub upload_indexes: &'a Vec<u32>,
    pub upload_view_matrices: &'a Vec<TMat4<f32>>
}

pub type DrawFunction = fn(&mut DrawParam);

/// Aggregate structure to hold all required information to build a render system
pub struct SystemInformation
{
    pub constant_values: Vec<Constant>,
    pub first_pass_vertex_shader: Option<VertexShaderInformation>,
    pub first_pass_fragment_shader: Option<FragmentShaderInformation>,
    pub second_pass_vertex_shader: Option<VertexShaderInformation>,
    pub second_pass_frag_shader: Option<FragmentShaderInformation>,
    pub indice_information: Option<IndiceInformation>,
    pub draw_function: Option<DrawFunction>,
    pub light_draw_function: Option<DrawFunction>,
    pub transparency_draw_function: Option<DrawFunction>,
    pub level_of_views: Vec<LevelOfView>,
    pub draw_fn_accessible_fbo: HashMap<String, FBO>,
    pub apply_lights: bool,
    pub max_num_lights: MaxNumLights,
    pub no_light_source_cutoff: f32,
    pub default_diffuse_factor: f32
}