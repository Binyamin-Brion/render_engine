use hashbrown::HashMap;
use crate::exports::load_models::MaxNumLights;
use crate::exports::rendering::LevelOfView;
use crate::helper_things::environment::{get_asset_folder, get_generated_shaders_folder};
use crate::models::model_definitions::MeshGeometry;
use crate::render_components::frame_buffer::FBO;
use crate::render_components::mapped_buffer::BufferWriteInfo;
use crate::render_system::initialize_logic::create_render_system;
use crate::render_system::render_system::RenderSystem;
use crate::render_system::system_information::{Constant, DrawFunction, FragmentShaderInformation, GLSLVersion, SystemInformation, Uniform, UniformBlock, UniformType, VertexShaderInformation};
use crate::specify_model_geometry_layouts;

/// Builder to start the process of creating a render system
pub struct RenderSystemBuilder(SystemInformation);

/// Builder to declare constant values in shader program
pub struct ConstantValuesBuilder(SystemInformation);

/// Builder to specify the vertex shader of a render system
pub struct FirstPassVertexShaderBuilder(SystemInformation);

/// Builder to specify the fragment shader of a render system
pub struct FirstPassFragmentShaderBuilder(SystemInformation);

/// Builder to specify the vertex shader of a render system
pub struct SecondPassVertexShaderBuilder(SystemInformation);

/// Builder to specify the fragment shader of a render system
pub struct SecondPassFragmentShaderBuilder(SystemInformation);

/// Builder to specify the draw function of a render system
pub struct DrawFunctionBuilder(SystemInformation);

/// Builder to specify the level of views of a render system
pub struct LevelOfViewsBuilder(SystemInformation);

/// Builder to specify what FBOs are available from the draw function
pub struct DrawFnAccessibleFBO(SystemInformation);

/// Builder to specify if lights should be rendered and alongside that, shadows
pub struct ApplyLightUploads(SystemInformation);

/// Builder to store information about the maximum number of lights in a scene
pub struct SpecifyLightNumberConstraint(SystemInformation);

pub struct NoLightDiffuseParam(SystemInformation);

/// Builder to create the render system
pub struct CreateRenderSystemBuilder(SystemInformation);

pub enum MaxLightConstraints
{
    Constraints(MaxNumLights),
    NotApplicable
}

// Below functions should be self-explanatory; comments are omitted

specify_model_geometry_layouts!(second_pass_update_fn,);

impl RenderSystemBuilder
{
    pub fn new() -> ConstantValuesBuilder
    {
        let max_num_lights = MaxNumLights
        {
            directional: 0,
            point: 0,
            spot: 0
        };

        ConstantValuesBuilder
            (
                SystemInformation
                {
                    constant_values: vec![],
                    first_pass_vertex_shader: None,
                    first_pass_fragment_shader: None,
                    second_pass_vertex_shader: None,
                    second_pass_frag_shader: None,
                    indice_information: None,
                    draw_function: None,
                    light_draw_function: None,
                    transparency_draw_function: None,
                    level_of_views: vec![],
                    draw_fn_accessible_fbo: HashMap::new(),
                    apply_lights: false,
                    max_num_lights,
                    no_light_source_cutoff: 0.0,
                    default_diffuse_factor: 0.0
                }
            )
    }
}

impl ConstantValuesBuilder
{
    pub fn with_constants(mut self, constants: Vec<Constant>) -> FirstPassVertexShaderBuilder
    {
        self.0.constant_values = constants;
        FirstPassVertexShaderBuilder(self.0)
    }
}

impl FirstPassVertexShaderBuilder
{
    pub fn with_vertex_shader(mut self, vertex_shader: VertexShaderInformation) -> FirstPassFragmentShaderBuilder
    {
        self.0.first_pass_vertex_shader = Some(vertex_shader);
        FirstPassFragmentShaderBuilder(self.0)
    }
}

impl FirstPassFragmentShaderBuilder
{
    pub fn with_first_pass_fragment_shader(mut self, fragment_shader: FragmentShaderInformation) -> SecondPassVertexShaderBuilder
    {
        self.0.first_pass_fragment_shader = Some(fragment_shader);
        SecondPassVertexShaderBuilder(self.0)
    }
}

impl SecondPassVertexShaderBuilder
{
    pub fn with_no_deferred_rendering(self) -> DrawFunctionBuilder
    {
        DrawFunctionBuilder(self.0)
    }

    pub fn with_second_pass_vertex_shader(mut self) -> SecondPassFragmentShaderBuilder
    {
        // With deferred rendering, the second pass vertex shader is hard coded
        self.0.second_pass_vertex_shader = Some(VertexShaderInformation
        {
            write_generated_shader: Some(get_generated_shaders_folder().join("second_pass_vertex.glsl").to_str().unwrap().to_string()),
            glsl_version: GLSLVersion::Core430,
            shader_source: get_asset_folder().join("shaders/second_pass_vertex.glsl"),
            layout_info: vec![],
            uniforms: vec![
                UniformBlock::new("lightInfo", 3, vec![
                    Uniform::new("projViewMatrix", UniformType::Mat4x4Float),
                    Uniform::new("renderingLightVolumes", UniformType::UInt)
                ]),
            ],
            instance_layout_update_fn: None,
            model_layout_update_fn: second_pass_update_fn,
            indice_buffers: None,
            out_variables: vec![],
            textures: vec![],
            cubemaps: vec![]
        });
        SecondPassFragmentShaderBuilder(self.0)
    }
}

impl SecondPassFragmentShaderBuilder
{
    pub fn with_second_pass_fragment_shader(mut self, fragment_shader: FragmentShaderInformation) -> DrawFunctionBuilder
    {
        self.0.second_pass_frag_shader = Some(fragment_shader);
        DrawFunctionBuilder(self.0)
    }
}

impl DrawFunctionBuilder
{
    pub fn with_draw_functions(mut self, draw_fn: DrawFunction, light_draw_function: DrawFunction,
                               transparency_draw_function: DrawFunction) -> LevelOfViewsBuilder
    {
        self.0.draw_function = Some(draw_fn);
        self.0.light_draw_function = Some(light_draw_function);
        self.0.transparency_draw_function = Some(transparency_draw_function);
        LevelOfViewsBuilder(self.0)
    }
}

impl LevelOfViewsBuilder
{
    pub fn with_level_of_views(mut self, level_of_views: Vec<LevelOfView>) -> DrawFnAccessibleFBO
    {
        self.0.level_of_views = level_of_views;
        DrawFnAccessibleFBO(self.0)
    }
}

impl DrawFnAccessibleFBO
{
    pub fn with_accessible_fbos(mut self, fbos: Vec<(String, FBO)>) -> ApplyLightUploads
    {
        for (name, fbo) in fbos
        {
            self.0.draw_fn_accessible_fbo.insert(name, fbo);
        }
        ApplyLightUploads(self.0)
    }
}

impl ApplyLightUploads
{
    pub fn apply_nearby_lights(mut self) -> SpecifyLightNumberConstraint
    {
        self.0.apply_lights = true;
        SpecifyLightNumberConstraint(self.0)
    }

    pub fn do_not_apply_nearby_lights(mut self) -> SpecifyLightNumberConstraint
    {
        self.0.apply_lights = false;
        SpecifyLightNumberConstraint(self.0)
    }
}

impl SpecifyLightNumberConstraint
{
    pub fn with_light_constraints(mut self, constraints: MaxLightConstraints) -> NoLightDiffuseParam
    {
        if let MaxLightConstraints::Constraints(constraints) = constraints
        {
            self.0.max_num_lights = constraints;
        }
        NoLightDiffuseParam(self.0)
    }
}

impl NoLightDiffuseParam
{
    pub fn with_no_light_diffuse_param(mut self, no_light_source_cutoff: f32, default_diffuse_factor: f32) -> CreateRenderSystemBuilder
    {
        self.0.no_light_source_cutoff = no_light_source_cutoff;
        self.0.default_diffuse_factor = default_diffuse_factor;
        CreateRenderSystemBuilder(self.0)
    }
}

impl CreateRenderSystemBuilder
{
    pub fn build(self) -> RenderSystem
    {
        create_render_system(self.0)
    }
}