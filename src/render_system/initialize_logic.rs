use std::any::TypeId;
use std::mem::size_of;
use hashbrown::HashMap;
use nalgebra_glm::{TMat4, TMat4x4, TVec2, TVec3, TVec4, vec2, vec3, vec4};
use crate::objects::ecs::ECS;
use crate::models::model_definitions::MeshGeometry;
use crate::render_components::cubemap::CubeMap;
use crate::render_components::frame_buffer::FBO;
use crate::render_components::mapped_buffer::{BindingInformation, BufferType, BufferWriteInfo, MappedBuffer};
use crate::render_components::shader_program::{ShaderInitInformation, ShaderProgram};
use crate::render_components::texture_array::TextureArray;
use crate::render_components::vao::VAO;
use crate::render_system::helper_constructs::ERROR_TEXTURE_COLOURS;
use crate::render_system::render_pass_resources::*;
use crate::render_system::render_system::RenderSystem;
use crate::render_system::system_information::*;
use crate::specify_model_geometry_layouts;

type TextureArrayIndex = usize;

specify_model_geometry_layouts!(second_pass_update_fn,);

/// Holds variables required to generated render pass resources; helper structure to reduce number
/// of parameters passed into functions
struct RenderPassInitArgs<'a>
{
    system_information: &'a SystemInformation,
    vertex_shader: &'a VertexShaderInformation,
    frag_shader: &'a FragmentShaderInformation,
    g_buffer_textures: &'a mut GBufferLayouts
}

/// Holds the generated parts of a shader to assemble together to create a
/// compiled OpenGL Vertex shader
struct DynamicVertexShaderGeneration
{
    generated_name: Option<String>,
    glsl_version: String,
    constants: String,
    layout: String,
    out_variables: String,
    texture_layouts: String,
    uniforms: String,
}

/// Holds the generated parts of a shader to assemble together to create a
/// compiled OpenGL Fragment shader
struct DynamicFragmentShaderGeneration
{
    generated_name: Option<String>,
    glsl_version: String,
    constants: String,
    layout: String,
    in_variables: String,
    out_variables: String,
    texture_layouts: String,
    uniforms: String,
}

impl DynamicVertexShaderGeneration
{
    /// Creates a new structure that assumes an empty vertex shader
    ///
    /// `generated_name` - the name of the file that will hold the generated shader
    fn new(generated_name: Option<String>) -> DynamicVertexShaderGeneration
    {
        DynamicVertexShaderGeneration
        {
            generated_name,
            glsl_version: "".to_string(),
            constants: "".to_string(),
            layout: "".to_string(),
            out_variables: "".to_string(),
            texture_layouts: "".to_string(),
            uniforms: "".to_string()
        }
    }

    /// Converts the internal representation of a vertex shader into a single string
    pub fn to_string(&self) -> String
    {
        let mut append_contents = self.glsl_version.clone() + "\n";
        append_contents += &(self.constants.clone() + "\n");
        append_contents += &(self.layout.clone() + "\n");
        append_contents += &(self.out_variables.clone() + "\n");
        append_contents += &(self.texture_layouts.clone() + "\n");
        append_contents += &(self.uniforms.clone() + "\n");
        append_contents
    }
}

impl DynamicFragmentShaderGeneration
{
    /// Creates a new structure that assumes an empty fragment shader
    ///
    /// `generated_name` - the name of the file that will hold the generated shader
    fn new(generated_name: Option<String>) -> DynamicFragmentShaderGeneration
    {
        DynamicFragmentShaderGeneration
        {
            generated_name,
            glsl_version: "".to_string(),
            constants: "".to_string(),
            layout: "".to_string(),
            in_variables: "".to_string(),
            out_variables: "".to_string(),
            texture_layouts: "".to_string(),
            uniforms: "".to_string()
        }
    }

    /// Converts the internal representation of a fragment shader into a single string
    fn to_string(&self) -> String
    {
        let mut append_contents = self.glsl_version.clone() + "\n";
        append_contents += &(self.constants.clone() + "\n");
        append_contents += &(self.layout.clone() + "\n");
        append_contents += &(self.in_variables.clone() + "\n");
        append_contents += &(self.out_variables.clone() + "\n");
        append_contents += &(self.texture_layouts.clone() + "\n");
        append_contents += &(self.uniforms.clone() + "\n");
        append_contents
    }
}

/// Stores the required layout code for the second pass GBuffer
pub struct GBufferLayouts
{
    layouts: String,
    number_layouts: u32,
}

/// Creates a render system with the provided information specified in the system_information
///
/// `system_information` - the information that specifies the content of shaders and required resources
///                        to use those shaders to create
pub fn create_render_system(system_information: SystemInformation) -> RenderSystem
{
    let first_render_pass_resources;
    let mut second_render_pass_resources = None;
    let mut g_buffer_layouts = GBufferLayouts{ layouts: "".to_string(), number_layouts: 0 };

    // There will always be a first-pass, otherwise the render system is invalid. Hence the panic in
    // the second branch arm. It is not required to have a second pass though
    match (&system_information.first_pass_vertex_shader, &system_information.first_pass_fragment_shader)
    {
        (Some(vertex_shader), Some(frag_shader)) =>
            {
                let render_system_init_args = RenderPassInitArgs
                {
                    system_information: &system_information,
                    vertex_shader,
                    frag_shader,
                    g_buffer_textures: &mut g_buffer_layouts
                };

                first_render_pass_resources = Some(create_first_render_pass_resources(render_system_init_args));
            },
        _ => panic!()
    }

    match (&system_information.second_pass_vertex_shader, &system_information.second_pass_frag_shader)
    {
        (Some(vertex_shader), Some(frag_shader)) =>
            {
                let render_system_init_args = RenderPassInitArgs
                {
                    system_information: &system_information,
                    vertex_shader,
                    frag_shader,
                    g_buffer_textures: &mut g_buffer_layouts
                };

                second_render_pass_resources = Some(create_second_render_pass_resources(render_system_init_args));
            },
        _ => {}
    }

    RenderSystem::new(first_render_pass_resources.unwrap(), second_render_pass_resources,
                      system_information.draw_function.unwrap(), system_information.light_draw_function.unwrap(),
                      system_information.transparency_draw_function.unwrap(), system_information.level_of_views,
                      system_information.draw_fn_accessible_fbo, system_information.apply_lights,
                      system_information.max_num_lights, system_information.no_light_source_cutoff,
                      system_information.default_diffuse_factor)
}

/// Creates the resources required for the first render pass of the render system
///
/// `render_system_init_args` - structure holding the parameters required to generate a shader and
///                             required OpenGL resources to use that shader
fn create_first_render_pass_resources(render_system_init_args: RenderPassInitArgs) -> RenderPassResources
{
    let mut dynamic_vertex_shader = DynamicVertexShaderGeneration::new(render_system_init_args.vertex_shader.write_generated_shader.clone());
    let mut dynamic_frag_shader = DynamicFragmentShaderGeneration::new(render_system_init_args.frag_shader.write_generated_shader.clone());

    dynamic_vertex_shader.glsl_version = render_system_init_args.vertex_shader.glsl_version.to_string();
    dynamic_frag_shader.glsl_version = render_system_init_args.frag_shader.glsl_version.to_string();

    extract_shared_constants(&render_system_init_args.system_information.constant_values, &mut dynamic_vertex_shader, &mut dynamic_frag_shader);
    extract_shared_variables(&render_system_init_args.vertex_shader, &render_system_init_args.frag_shader, &mut dynamic_vertex_shader, &mut dynamic_frag_shader);

    let deferred_rendering_fbo = if render_system_init_args.frag_shader.layouts.is_empty()
    {
        None
    }
    else
    {
        Some(extract_frag_layouts(&render_system_init_args.frag_shader, &mut dynamic_frag_shader, render_system_init_args.g_buffer_textures))
    };

    let shadow_map_binding_point = extract_textures(&render_system_init_args.vertex_shader, &render_system_init_args.frag_shader, &mut dynamic_vertex_shader, &mut dynamic_frag_shader);

    extract_uniforms(&render_system_init_args.vertex_shader, &render_system_init_args.frag_shader, &mut dynamic_vertex_shader, &mut dynamic_frag_shader);

    let mut vao = VAO::new();
    let vertex_shader_resource =    create_first_pass_vertex_resources(&render_system_init_args.vertex_shader, &mut vao, &mut dynamic_vertex_shader);
    let fragment_shader_resource = extract_frag_texture_resources(&render_system_init_args.frag_shader);
    let uniform_resources = create_padded_uniform_block(&render_system_init_args.vertex_shader, &render_system_init_args.frag_shader);
    let shader_program = create_shader_program(&render_system_init_args.vertex_shader, &render_system_init_args.frag_shader, dynamic_vertex_shader, dynamic_frag_shader);

    RenderPassResources
    {
        shader_program,
        vao,
        vertex_shader_resource,
        fragment_shader_resource,
        uniform_resources,
        uploaded_textures: HashMap::new(),
        shadow_map_binding_point,
        deferred_rendering_fbo
    }
}

/// Creates the resources required for the second render pass of the render system
///
/// `render_system_init_args` - structure holding the parameters required to generate a shader and
///                             required OpenGL resources to use that shader
fn create_second_render_pass_resources(render_system_init_args: RenderPassInitArgs) -> RenderPassResources
{
    let mut dynamic_vertex_shader = DynamicVertexShaderGeneration::new(render_system_init_args.vertex_shader.write_generated_shader.clone());
    let mut dynamic_frag_shader = DynamicFragmentShaderGeneration::new(render_system_init_args.frag_shader.write_generated_shader.clone());
    dynamic_frag_shader.layout = render_system_init_args.g_buffer_textures.layouts.clone();

    dynamic_vertex_shader.glsl_version = render_system_init_args.vertex_shader.glsl_version.to_string();
    dynamic_frag_shader.glsl_version = render_system_init_args.frag_shader.glsl_version.to_string();

    extract_shared_constants(&render_system_init_args.system_information.constant_values, &mut dynamic_vertex_shader, &mut dynamic_frag_shader);
    extract_shared_variables(&render_system_init_args.vertex_shader, &render_system_init_args.frag_shader, &mut dynamic_vertex_shader, &mut dynamic_frag_shader);

    let deferred_rendering_fbo = None;
    let shadow_map_binding_point = if render_system_init_args.frag_shader.include_shadow_maps
    {
        // Indexes start at 0, hence why number_layouts does not have a +1
        dynamic_frag_shader.layout += &format!("layout (binding = {}) uniform sampler2DArray shadowMaps;\n", render_system_init_args.g_buffer_textures.number_layouts);
        Some(render_system_init_args.g_buffer_textures.number_layouts)
    }
    else
    {
        None
    };

    extract_uniforms(&render_system_init_args.vertex_shader, &render_system_init_args.frag_shader, &mut dynamic_vertex_shader, &mut dynamic_frag_shader);
    let mut vao = VAO::new();
    let vertex_shader_resource =  create_second_pass_vertex_resources(&mut vao);
    let fragment_shader_resource = extract_frag_texture_resources(&render_system_init_args.frag_shader);

    let uniform_resources = create_padded_uniform_block(&render_system_init_args.vertex_shader, &render_system_init_args.frag_shader);
    let shader_program = create_shader_program(&render_system_init_args.vertex_shader, &render_system_init_args.frag_shader, dynamic_vertex_shader, dynamic_frag_shader);

    RenderPassResources
    {
        shader_program,
        vao,
        vertex_shader_resource,
        fragment_shader_resource,
        uniform_resources,
        uploaded_textures: HashMap::new(),
        shadow_map_binding_point,
        deferred_rendering_fbo
    }
}

/// Finds that required fragment layouts to write to the g-buffer
///
/// `frag_info` - layout information to include in the generated shader as well as associated resources to create
/// `dynamic_frag` - location to store the generated shader g-buffer layout code for the first render pass
/// `g_buffer_textures` - location to store texture bindings of the g-buffer for the second pass
fn extract_frag_layouts(frag_info: &FragmentShaderInformation, dynamic_frag: &mut DynamicFragmentShaderGeneration, g_buffer_textures: &mut GBufferLayouts) -> FBO
{
    let mut colour_attachments = vec![];
    g_buffer_textures.number_layouts = frag_info.layouts.len() as u32;

    for (index, layout) in frag_info.layouts.iter().enumerate()
    {
        dynamic_frag.layout += &format!("layout (location = {}) out {} {};\n", index, layout.data_type.to_string(), layout.name);

        // The first pass writes to the g-buffer using layouts. The second pass uses the g-buffer through
        // texture bindings. Need to have the same number of layouts and bindings- same name for layout and
        // and bindings helps with readability
        g_buffer_textures.layouts += &format!("layout (binding = {}) uniform sampler2DArray {};\n", index, layout.name);

        let texture_information = TextureInformation
        {
            sampler_name: layout.name.clone(),
            number_mipmaps: 1,
            format: layout.backing_texture_format,
            min_filter_options: MinFilterOptions::Nearest,
            mag_filter_options: MagFilterOptions::Nearest,
            wrap_s: TextureWrap::ClampToEdge,
            wrap_t: TextureWrap::ClampToEdge,
            width: layout.initial_fbo_size.0,
            height: layout.initial_fbo_size.1,
            number_textures: 1,
            border_color: None
        };

        colour_attachments.push(texture_information);
    }

    // Depth maps are not specified in the render system declaration; they are automatically added
    let depth_information = TextureInformation
    {
        sampler_name: "depthMap".to_string(),
        number_mipmaps: 1,
        format: TextureFormat::DepthStencil,
        min_filter_options: MinFilterOptions::Nearest,
        mag_filter_options: MagFilterOptions::Nearest,
        wrap_s: TextureWrap::ClampToEdge,
        wrap_t: TextureWrap::ClampToEdge,
        width: colour_attachments.last().unwrap().width,
        height: colour_attachments.last().unwrap().height,
        number_textures: 1,
        border_color: None
    };

    FBO::new(colour_attachments, None, None, Some(depth_information)).unwrap()

}

/// ******************* Shader Program Functions *************************

/// Create a shader program from the given system information
///
/// `vertex_shader` - structure holding the location of the file that has the logic to append to the
///                     generated vertex shader inputs and outputs
/// `frag_shader_info` - structure holding the location of the file that has the logic to append to the
///                     generated fragment shader inputs and outputs
/// `dynamic_vertex` - structure holding the generated shader source for the vertex shader
/// `dynamic_frag` - structure holding the generated shader source for the vertex shader
fn create_shader_program(vertex_shader_info: &VertexShaderInformation, frag_shader_info: &FragmentShaderInformation,
                         dynamic_vertex: DynamicVertexShaderGeneration, dynamic_frag: DynamicFragmentShaderGeneration) -> ShaderProgram
{
    let mut shaders_init_information = Vec::new();

    let vertex_shader_source = vertex_shader_info.shader_source.clone();
    let vertex_init_info = ShaderInitInformation::from_file(gl::VERTEX_SHADER,vertex_shader_source, Some(dynamic_vertex.to_string()), dynamic_vertex.generated_name).unwrap();
    shaders_init_information.push(vertex_init_info);

    let fragment_shader_source = frag_shader_info.shader_source.clone();
    let fragment_init_info = ShaderInitInformation::from_file(gl::FRAGMENT_SHADER,fragment_shader_source, Some(dynamic_frag.to_string()), dynamic_frag.generated_name).unwrap();
    shaders_init_information.push(fragment_init_info);

    ShaderProgram::new(&shaders_init_information).unwrap()
}

/// Creates the shader code to use constant variables
///
/// `constants` - the constants that are used in the shader program of a given render pass
/// `dynamic_vertex` - location to store generated shader code for constant variables in the vertex shader
/// `dynamic_frag` - location to store generated shader code for constant variables in the fragment shader
fn extract_shared_constants(constants: &Vec<Constant>, dynamic_vertex: &mut DynamicVertexShaderGeneration, dynamic_frag: &mut DynamicFragmentShaderGeneration)
{
    for constant in constants
    {
        let (data_type, data_value) = constant.value.to_string();

        for target in &constant.share_targets
        {
            match *target
            {
                ConstantLocation::VertexShader => dynamic_vertex.constants += &format!("const {} {} = {};\n", data_type, constant.name, data_value),
                ConstantLocation::FragmentShader => dynamic_frag.constants += &format!("const {} {} = {};\n", data_type, constant.name, data_value),
            }
        }
    }
}

/// Creates the shader code to use in/out variables
///
/// `vertex_shader` - structure containing the in/out variables for the vertex shader of a render pass
/// `frag_shader` - structure containing the in/out variables for the fragment shader of a render pass
/// `dynamic_vertex` - location to store generated shader code for in/out variables in the vertex shader
/// `dynamic_frag` - location to store generated shader code for in/out  variables in the fragment shader
fn extract_shared_variables(vertex_shader: &VertexShaderInformation, frag_shader: &FragmentShaderInformation,
                            dynamic_vertex: &mut DynamicVertexShaderGeneration, dynamic_frag: &mut DynamicFragmentShaderGeneration)
{
    let array_info = |data_type: SharedVariableType|
        {
            match data_type
            {
                SharedVariableType::Mat4Array(i) | SharedVariableType::Vec4Array(i) |
                SharedVariableType::Vec3Array(i) | SharedVariableType::FloatArray(i) |
                SharedVariableType::UIntArray(i) => format!(" [{}];", i),
                _ => ";".to_string()
            }
        };

    let flat_info = |is_flat: bool|
        {
            match is_flat
            {
                true => "flat ",
                false => ""
            }
        };

    // If the vertex shader has out variables, then they must lead to somewhere- in this case, since
    // the geometry shader is not available, these must lead to the fragment shader
    for x in &vertex_shader.out_variables
    {
        dynamic_vertex.out_variables += &format!("{}out {} {}{}\n", flat_info(x.is_flat), x.data_type.to_string(), x.name, array_info(x.data_type));
        dynamic_frag.in_variables += &format!("{}in {} {}{}\n", flat_info(x.is_flat), x.data_type.to_string(), x.name, array_info(x.data_type));
    }

    // Realistically this only happens for the "out vec4 FragColor"
    for x in &frag_shader.out_variables
    {
        dynamic_frag.out_variables += &format!("{}out {} {}{}\n", flat_info(x.is_flat), x.data_type.to_string(), x.name, array_info(x.data_type));
    }
}

/// Generates the shader code to use textures
///
/// `vertex_shader` - structure containing the texture variables for the vertex shader of a render pass
/// `frag_shader` - structure containing the texture variables for the fragment shader of a render pass
/// `dynamic_vertex` - location to store generated shader code for texture variables in the vertex shader
/// `dynamic_frag` - location to store generated shader code for texture variables in the fragment shader
fn extract_textures(vertex_shader: &VertexShaderInformation, frag_shader: &FragmentShaderInformation,
                    dynamic_vertex: &mut DynamicVertexShaderGeneration, dynamic_frag: &mut DynamicFragmentShaderGeneration) -> Option<u32>
{
    let mut number_binding_points_processed = 0;

    if frag_shader.include_error_textures
    {
        dynamic_frag.texture_layouts += &format!("layout (binding = {}) uniform sampler2DArray errorTextureArray;\n", number_binding_points_processed);
        number_binding_points_processed += 1;
    }

    let shadow_map_binding_point = if frag_shader.include_shadow_maps
    {
        dynamic_frag.texture_layouts += &format!("layout (binding = {}) uniform sampler2DArray shadowMaps;\n", number_binding_points_processed);
        number_binding_points_processed += 1;
        Some(number_binding_points_processed - 1)
    }
    else
    {
        None
    };

    let mut add_texture_arrays = |textures: &Vec<TextureInformation>, cubemaps: &Vec<CubeMapInitInfo>, stoage: &mut String|
        {
            for x in textures
            {
                *stoage += &format!("layout (binding = {}) uniform sampler2DArray {};\n", number_binding_points_processed, x.sampler_name);
                number_binding_points_processed += 1;
            }

            for x in cubemaps
            {
                *stoage += &format!("layout (binding = {}) uniform samplerCube {};\n", number_binding_points_processed, x.cube_map_name);
                number_binding_points_processed += 1;
            }
        };

    // Theoretically a vertex shader could have textures
    add_texture_arrays(&vertex_shader.textures, &vertex_shader.cubemaps, &mut dynamic_vertex.texture_layouts);
    add_texture_arrays(&frag_shader.textures, &frag_shader.cubemaps, &mut dynamic_frag.texture_layouts);
    shadow_map_binding_point
}

/// Generates the shader code to use uniforms and put them in a uniform block
///
/// `vertex_shader` - structure containing the uniform variables for the vertex shader of a render pass
/// `frag_shader` - structure containing the uniform variables for the fragment shader of a render pass
/// `dynamic_vertex` - location to store generated shader code for uniform variables in the vertex shader
/// `dynamic_frag` - location to store generated shader code for uniform variables in the fragment shader
fn extract_uniforms(vertex_shader_uniforms: &VertexShaderInformation, frag_shader_uniforms: &FragmentShaderInformation,
                    dynamic_vertex: &mut DynamicVertexShaderGeneration, dynamic_frag: &mut DynamicFragmentShaderGeneration)
{
    let mut number_binding_points_processed = 0;

    let mut add_uniforms = |uniforms: &Vec<UniformBlock>, storage: &mut String|
        {
            for x in uniforms
            {
                let uniform_block_declaration = format!("layout (std140, binding = {}) uniform {}", number_binding_points_processed, x.block_name);
                let mut uniform_block_body = String::new();

                for uniform in &x.uniforms
                {
                    let array_info = match uniform.uniform_type
                    {
                        UniformType::Mat4Array(i) | UniformType::Vec4Array(i) |
                        UniformType::Vec3Array(i) | UniformType::FloatArray(i) |
                        UniformType::UIntArray(i) => format!(" [{}];", i),
                        _ => ";".to_string()
                    };

                    uniform_block_body += &format!("\t{} {}{}\n", uniform.uniform_type.to_string(), uniform.name, array_info);
                }

                // The series of braces is required as writing the uniform block requires writing
                // "{" and "}", and the number of braces written is the correct number using the format macro
                *storage += &format!("{}\n{{{}}};\n\n", uniform_block_declaration, uniform_block_body);
                number_binding_points_processed += 1;
            }
        };

    add_uniforms(&vertex_shader_uniforms.uniforms, &mut dynamic_vertex.uniforms);
    add_uniforms(&frag_shader_uniforms.uniforms, &mut dynamic_frag.uniforms);
}

/// *********** Vertex Shader Related Functions ***************

/// Stores the information required to write generated code for shader layouts and to create
/// backing buffers for those layouts
pub struct LayoutBindingInformation
{
    pub binding_info: Vec<BindingInformation>,
    pub num_layouts_used: u32,
    pub glsl_type: String,
}

/// Create the required OpenGL resources for the second-pass vertex shader
///
/// `vao` - the VAO that is to be used for the second pass rendering
fn create_second_pass_vertex_resources(vao: &mut VAO) -> VertexShaderResources
{
    // Information to render the g-buffer as a rectangle

    let vertices: Vec<TVec3<f32>> =
        vec![
            vec3(-1.0, -1.0, 0.0),
            vec3(-1.0, 1.0, 0.0),
            vec3(1.0, 1.0, 0.),
            vec3(1.0, -1.0, 0.0)
        ];

    let tex_coords: Vec<TVec2<f32>> =
        vec![
            vec2(0.0, 0.0),
            vec2(0.0, 1.0),
            vec2(1.0, 1.0),
            vec2(1.0, 0.0)
        ];

    let indices: Vec<u32> =
        vec![
            0, 1, 2,
            2, 0, 3
        ];

    vao.specify_layout_format(0, 3, gl::FLOAT, 0);
    vao.specify_layout_format(1, 2, gl::FLOAT, 0);

    let size_vertex = size_of::<TVec3<f32>>();
    let size_texcoord = size_of::<TVec2<f32>>();

    let size_vertices = size_vertex * vertices.len();
    let size_texcoords = size_texcoord * tex_coords.len();
    let size_indices = size_of::<u32>() * indices.len();

    let mut vertices_buffer = MappedBuffer::new(size_vertices as isize, BufferType::NonIndiceArray(vec![BindingInformation::new(0, 0, size_vertex as i32)]), 1);
    let mut texcoord_buffer = MappedBuffer::new(size_texcoords as isize, BufferType::NonIndiceArray(vec![BindingInformation::new(1, 0, size_texcoord as i32)]), 1);
    let mut indices_buffer = MappedBuffer::new(size_indices as isize, BufferType::IndiceArray, 1);

    let vertices_write_info = vertices_buffer.wait_for_next_free_buffer(5_000_000).unwrap();
    let texcoord_buffer_info = texcoord_buffer.wait_for_next_free_buffer(5_000_000).unwrap();
    let indices_buffer_info = indices_buffer.wait_for_next_free_buffer(5_000_000).unwrap();

    MappedBuffer::write_data_serialized(vertices_write_info, &vertices, 0, true);
    MappedBuffer::write_data_serialized(texcoord_buffer_info, &tex_coords, 0, true);
    MappedBuffer::write_data_serialized(indices_buffer_info, &indices, 0, true);

    // Make sure that the updated contents are recognised by OpenGL
    vertices_buffer.flush_entire_buffer();
    texcoord_buffer.flush_entire_buffer();
    indices_buffer.flush_entire_buffer();

    VertexShaderResources
    {
        indice_buffer: Some(indices_buffer),
        per_model_buffers: vec![vertices_buffer, texcoord_buffer],
        per_instance_buffers: vec![],
        layout_update_fn: None,
        model_update_fn: second_pass_update_fn,
        model_layout_indexes: vec![],
        instance_layout_indexes: vec![],
    }
}

/// Create the required OpenGL resources for the first-pass render pass
///
/// `vertex_shader` - the structure containing the layout information for the vertex shader
/// `vao` - the VAO that is to be used for the first-pass rendering
/// `dynamic_vertex` - structure to store the generated shader code the layouts in the vertex shader
fn create_first_pass_vertex_resources(vertex_shader: &VertexShaderInformation, vao: &mut VAO, dynamic_vertex: &mut DynamicVertexShaderGeneration) -> VertexShaderResources
{
    let mut per_model_buffers = Vec::new();
    let mut per_instance_buffers = Vec::new();

    let mut model_layout_indexes = Vec::new();
    let mut instance_layout_indexes = Vec::new();

    let mut layout_index = 0;

    for layout_info in vertex_shader.layout_info.iter()
    {
        let layout_binding_info = create_layout_binding_information(layout_info.data_type, layout_index, vao);

        let mapped_buffer = match layout_info.instance
        {
            LayoutInstance::Divisor0(number_buffers, size_buffer_bytes) =>
                {
                    // By default all layouts defined are Divisor0, so no need to explicitly set layout divisor
                    MappedBuffer::new(size_buffer_bytes, BufferType::NonIndiceArray(layout_binding_info.binding_info), number_buffers)
                },
            LayoutInstance::Divisor1(number_buffers, size_buffer_bytes) =>
                {
                    for count in 0..layout_binding_info.num_layouts_used
                    {
                        vao.specify_layout_divisor(layout_index + count, 1);
                    }

                    MappedBuffer::new(size_buffer_bytes, BufferType::NonIndiceArray(layout_binding_info.binding_info), number_buffers)
                }
        };

        match layout_info.layout_use
        {
            LayoutUse::PerModel =>
                {
                    model_layout_indexes.push(layout_index);
                    per_model_buffers.push(mapped_buffer);
                },
            LayoutUse::PerInstance =>
                {
                    instance_layout_indexes.push(layout_index);
                    per_instance_buffers.push(mapped_buffer);
                }
        }

        dynamic_vertex.layout += &format!("layout (location = {}) in {} {};\n", layout_index, layout_binding_info.glsl_type, layout_info.name);

        layout_index += layout_binding_info.num_layouts_used;
    }

    let indice_buffer =
        if let Some(indice_buffer_info) = vertex_shader.indice_buffers
        {
            Some(MappedBuffer::new(indice_buffer_info.buffer_size_bytes, BufferType::IndiceArray, indice_buffer_info.number_buffers))
        }
        else
        {
            None
        };

    VertexShaderResources
    {
        per_model_buffers,
        per_instance_buffers,
        layout_update_fn: vertex_shader.instance_layout_update_fn,
        model_update_fn: vertex_shader.model_layout_update_fn,
        model_layout_indexes,
        instance_layout_indexes,
        indice_buffer,
    }
}

/// Creates the information needed to bind a buffer to a binding point that is used by a VAO.
/// The layout also has its format and divisor set for the VAO
///
/// `layout` - the data that is to be represented by the layout
/// `index` - the index of the layout
/// `vao` - the VAO to use for the first-pass rendering
pub fn create_layout_binding_information(layout: LayoutType, index: u32, vao: &mut VAO) -> LayoutBindingInformation
{
    // NumberLayoutsUsed == Length of BindingInfo vec; it is returned for convenience purposes
    // Returned TypeId used to ensure correctly structure is used to upload data into a buffer

    // As of time of writing, all layouts get their own buffer. As a result, all buffer offsets (not VAO relative offsets) are 0

    // Could probably use macros to make code more concise; at time of writing this led to compiler errors

    return match layout
    {
        LayoutType::Vec2Uint =>
            {
                vao.specify_layout_format(index, 2, gl::UNSIGNED_INT, 0);
                LayoutBindingInformation
                {
                    binding_info: vec![BindingInformation::new(index, 0, (size_of::<TVec2<u32>>()) as i32)],
                    num_layouts_used: 1,
                    glsl_type: "uvec2".to_string()
                }
            },
        LayoutType::Vec2Int =>
            {
                vao.specify_layout_format(index, 2, gl::INT, 0);
                LayoutBindingInformation
                {
                    binding_info: vec![BindingInformation::new(index, 0, (size_of::<TVec2<i32>>()) as i32)],
                    num_layouts_used: 1,
                    glsl_type: "ivec2".to_string()
                }
            },
        LayoutType::Vec2Float =>
            {
                vao.specify_layout_format(index, 2, gl::FLOAT, 0);
                LayoutBindingInformation
                {
                    binding_info: vec![BindingInformation::new(index, 0, (size_of::<TVec2<f32>>()) as i32)],
                    num_layouts_used: 1,
                    glsl_type: "vec2".to_string()
                }
            },
        LayoutType::Vec3Float =>
            {
                vao.specify_layout_format(index, 3, gl::FLOAT, 0);
                LayoutBindingInformation
                {
                    binding_info: vec![BindingInformation::new(index, 0, (size_of::<TVec3<f32>>()) as i32)],
                    num_layouts_used: 1,
                    glsl_type: "vec3".to_string()
                }
            },
        LayoutType::Vec4Float =>
            {
                vao.specify_layout_format(index, 4, gl::FLOAT, 0);
                LayoutBindingInformation
                {
                    binding_info: vec![BindingInformation::new(index, 0, (size_of::<TVec4<f32>>()) as i32)],
                    num_layouts_used: 1,
                    glsl_type: "vec4".to_string()
                }
            },
        LayoutType::Vec3Uint =>
            {
                vao.specify_layout_format(index, 3, gl::UNSIGNED_INT, 0);
                LayoutBindingInformation
                {
                    binding_info: vec![BindingInformation::new(index, 0, (size_of::<TVec3<u32>>()) as i32)],
                    num_layouts_used: 1,
                    glsl_type: "uvec3".to_string()
                }
            }
        LayoutType::Vec4Uint =>
            {
                vao.specify_layout_format(index, 4, gl::UNSIGNED_INT, 0);
                LayoutBindingInformation
                {
                    binding_info: vec![BindingInformation::new(index, 0, (size_of::<TVec4<u32>>()) as i32)],
                    num_layouts_used: 1,
                    glsl_type: "uvec4".to_string()
                }
            }
        LayoutType::Mat4x4Float =>
            {
                let size_vec4 = size_of::<TVec4<f32>>() as u32;
                let size_mat4 = size_of::<TMat4<f32>>() as i32;

                vao.specify_layout_format(index, 4, gl::FLOAT, 0);
                vao.specify_layout_format(index + 1, 4, gl::FLOAT, size_vec4);
                vao.specify_layout_format(index + 2, 4, gl::FLOAT, size_vec4 * 2);
                vao.specify_layout_format(index + 3, 4, gl::FLOAT, size_vec4 * 3);

                LayoutBindingInformation
                {
                    binding_info: vec![
                        BindingInformation::new(index, 0, size_mat4),
                        BindingInformation::new(index + 1, 0, size_mat4),
                        BindingInformation::new(index + 2, 0, size_mat4),
                        BindingInformation::new(index + 3, 0, size_mat4)
                    ],
                    num_layouts_used: 4,
                    glsl_type: "mat4".to_string(),
                }
            },
    }
}

/// ************ Fragment Shader Related Functions ****************

/// Creates texture resources for the texture specified for the fragment shader
///
/// `frag_shader` - the structure holding the texture information for the fragment shader (either first or second pass)
fn extract_frag_texture_resources(frag_shader: &FragmentShaderInformation) -> FragmentShaderResources
{
    let adjust_binding_points_shadows = if frag_shader.include_shadow_maps
    {
        1
    }
    else
    {
        0
    };

    let (texture_arrays, texture_lookup) = create_texture_array(frag_shader, adjust_binding_points_shadows);
    let cube_maps = create_cubemaps(frag_shader, adjust_binding_points_shadows + texture_arrays.len() as u32);

    FragmentShaderResources { texture_arrays, texture_lookup, cube_maps }
}

/// Creates texture arrays for the passed in fragment shader
///
/// `frag_shader` - the structure containing the texture array information for the fragment shader
/// `starting_layout_index` - starting index for texture arrays; if shadow maps are being included
///                             in the fragment shader then texture array indexes need to be changed
fn create_texture_array(frag_shader: &FragmentShaderInformation, starting_layout_index: u32) -> (Vec<TextureArray>, HashMap<String, TextureArrayIndex>)
{
    let mut texture_arrays = Vec::new();
    let mut texture_array_lookup = HashMap::new();

    if frag_shader.include_error_textures
    {
        // Create the default error textures
        let texture_info = TextureInformation
        {
            sampler_name: "errorTextures".to_string(),
            number_mipmaps: 1,
            format: TextureFormat::RGBA,
            width: 1,
            height: 1,
            number_textures: ERROR_TEXTURE_COLOURS.len() as i32, // One for each of: diffuse, dissolve, normal, shininess, specular, texture found but cannot upload,
            min_filter_options: MinFilterOptions::Linear,
            mag_filter_options: MagFilterOptions::Linear,
            wrap_s: TextureWrap::MirroredRepeat,
            wrap_t: TextureWrap::MirroredRepeat,
            border_color: None,
        };
        let mut texture_array = TextureArray::new(texture_info.clone(), 1, texture_arrays.len() as u32);
        for x in 0..ERROR_TEXTURE_COLOURS.len()
        {
            texture_array.add_texture_solid_colour(ERROR_TEXTURE_COLOURS[x]);
        }
        texture_array_lookup.insert("errorTextures".to_string(), texture_arrays.len());
        texture_array.bind_texture_to_texture_unit();
        texture_arrays.push(texture_array);
    }

    for texture_info in &frag_shader.textures
    {
        // There may be more than one texture array in the rendering system, so a map to find the index
        // of the texture array based off of its sampler name is needed
        texture_array_lookup.insert(texture_info.sampler_name.clone(), texture_arrays.len());

        // Binding point must match that in the shader. Remember that binding point used in the code is implicit- it is based off of the
        // index of the current texture array of all texture arrays defined
        let mut texture_array = TextureArray::new(texture_info.clone(), 1, starting_layout_index + texture_arrays.len() as u32);
        texture_array.bind_texture_to_texture_unit();
        texture_arrays.push(texture_array);
    }

    (texture_arrays, texture_array_lookup)
}

/// Creates cubemaps for the passed in fragment shader
///
/// `frag_shader` - the structure containing the texture array information for the fragment shader
/// `starting_layout_index` - starting index for cubemaps; the starting index starts after the last index
///                             used for texture arrays
fn create_cubemaps(frag_shader: &FragmentShaderInformation, starting_layout_index: u32) -> HashMap<String, CubeMap>
{
    let mut cubemap_lookup = HashMap::new();
    let mut number_cube_maps_made = 0;

    for cubemap in &frag_shader.cubemaps
    {
        let cube_map_resource = CubeMap::new(starting_layout_index + number_cube_maps_made);

        cubemap_lookup.insert(cubemap.cube_map_name.clone(), cube_map_resource);

        number_cube_maps_made += 1;
    }

    cubemap_lookup
}

/// *********************** Uniform Related Functions ***********************

/// Holds the information to know what buffer data is stored in, and what offset within that buffer.
#[derive(Debug)]
pub struct UniformDataLocation
{
    pub mapped_buffer_index: usize,
    pub offset_bytes: isize,
    pub sub_padding_bytes: isize // For uniform arrays
}

/// Stores the expected data to be uploaded to a uniform for error checking
pub struct ExpectedUniformData
{
    pub type_id: TypeId,
    pub num_elements: u16
}

/// Creates a single array with space reserved for all uniforms specified for the render system. The array
/// pads each uniform to ensure proper alignment
///
/// `vertex_shader_uniforms` - the uniform blocks declared in a vertex shader
/// `fragment_shader_uniforms` - the uniform blocks declared in a fragment shader
fn create_padded_uniform_block(vertex_shader_uniforms: &VertexShaderInformation, frag_shader_uniforms: &FragmentShaderInformation) -> UniformResources
{
    let mut mapped_buffers = Vec::new();
    let mut uniform_location_map = HashMap::new();

    // Stores what structure needs to be the source to update a uniform- ie, a Mat4x4 uniform needs
    // a TMat4x4<f32> (nalgebra-glm) structure to be used as the source for updating that uniform
    let mut uniform_type_ids = HashMap::new();

    let mut ecs = ECS::new();
    ecs.register_type::<UniformVec3>();
    ecs.register_type::<UniformMat4>();
    ecs.register_type::<UniformInt>();
    ecs.register_type::<UniformUint>();
    ecs.register_type::<UniformVec3Array>();
    ecs.register_type::<UniformFloat>();
    ecs.register_type::<UniformFloatArray>();
    ecs.register_type::<UniformVec4>();
    ecs.register_type::<UniformVec4Array>();
    ecs.register_type::<UniformMat4Array>();
    ecs.register_type::<UniformUIntArray>();
    let mut uniform_entities  = HashMap::new();

    let alignment_scalar = 4;
    let alignment_mat4x4_float = 16;

    let all_uniforms_blocks =
        {
            let mut vertex_shader_uniforms = vertex_shader_uniforms.uniforms.clone();
            vertex_shader_uniforms.extend(frag_shader_uniforms.uniforms.clone());
            vertex_shader_uniforms
        };

    // All uniforms are in uniform blocks
    for uniform_block in all_uniforms_blocks
    {
        let mut uniform_buffer_size = 0;

        for uniform in &uniform_block.uniforms
        {
            let size_uniform = Uniform::size_uniform_bytes(uniform.uniform_type);
            let sub_padding_bytes;

            match uniform.uniform_type
            {
                // Could probably use macros to make code more concise; at time of writing this led to compiler errors

                // Pad the array to ensure proper alignment. After this point, the uniform_buffer_size
                // will hold the starting index into the array for the current uniform being processed
                UniformType::Vec3 =>
                    {
                        uniform_buffer_size += padding_required(uniform_buffer_size, alignment_mat4x4_float);

                        let entity_id = ecs.create_entity();
                        ecs.write_component::<UniformVec3>(entity_id, UniformVec3(vec3(0.0, 0.0, 0.0)));

                        let expected_source_info = ExpectedUniformData
                        {
                            type_id: TypeId::of::<TVec3<f32>>(),
                            num_elements: 1
                        };

                        uniform_type_ids.insert(uniform.name.clone(), expected_source_info);
                        uniform_entities.insert(uniform.name.clone(), entity_id);

                        sub_padding_bytes = 0;
                    },
                UniformType::Mat4x4Float =>
                    {
                        uniform_buffer_size += padding_required(uniform_buffer_size, alignment_mat4x4_float);

                        let entity_id = ecs.create_entity();
                        ecs.write_component::<UniformMat4>(entity_id, UniformMat4(nalgebra_glm::identity()));

                        let expected_source_info = ExpectedUniformData
                        {
                            type_id: TypeId::of::<TMat4x4<f32>>(),
                            num_elements: 1
                        };

                        uniform_type_ids.insert(uniform.name.clone(), expected_source_info);
                        uniform_entities.insert(uniform.name.clone(), entity_id);

                        sub_padding_bytes = 0;
                    },
                UniformType::UInt =>
                    {
                        uniform_buffer_size += padding_required(uniform_buffer_size, alignment_scalar);

                        let entity_id = ecs.create_entity();
                        ecs.write_component::<UniformUint>(entity_id, UniformUint(0));

                        let expected_source_info = ExpectedUniformData
                        {
                            type_id: TypeId::of::<u32>(),
                            num_elements: 1
                        };

                        uniform_type_ids.insert(uniform.name.clone(), expected_source_info);
                        uniform_entities.insert(uniform.name.clone(), entity_id);

                        sub_padding_bytes = 0;
                    },
                UniformType::Int =>
                    {
                        uniform_buffer_size += padding_required(uniform_buffer_size, alignment_scalar);

                        let entity_id = ecs.create_entity();
                        ecs.write_component::<UniformInt>(entity_id, UniformInt(0));

                        let expected_source_info = ExpectedUniformData
                        {
                            type_id: TypeId::of::<i32>(),
                            num_elements: 1
                        };

                        uniform_type_ids.insert(uniform.name.clone(), expected_source_info);
                        uniform_entities.insert(uniform.name.clone(), entity_id);

                        sub_padding_bytes = 0;
                    },
                UniformType::FloatArray(num_elements) =>
                    {
                        uniform_buffer_size += padding_required(uniform_buffer_size, alignment_mat4x4_float);

                        let entity_id = ecs.create_entity();
                        ecs.write_component::<UniformFloatArray>(entity_id, UniformFloatArray(vec![0.0; num_elements as usize]));

                        let expected_source_info = ExpectedUniformData
                        {
                            type_id: TypeId::of::<f32>(),
                            num_elements
                        };

                        uniform_type_ids.insert(uniform.name.clone(), expected_source_info);
                        uniform_entities.insert(uniform.name.clone(), entity_id);

                        sub_padding_bytes = 12;
                    },
                UniformType::UIntArray(num_elements) =>
                    {
                        uniform_buffer_size += padding_required(uniform_buffer_size, alignment_mat4x4_float);

                        let entity_id = ecs.create_entity();
                        ecs.write_component::<UniformUIntArray>(entity_id, UniformUIntArray(vec![0; num_elements as usize]));

                        let expected_source_info = ExpectedUniformData
                        {
                            type_id: TypeId::of::<u32>(),
                            num_elements
                        };

                        uniform_type_ids.insert(uniform.name.clone(), expected_source_info);
                        uniform_entities.insert(uniform.name.clone(), entity_id);

                        sub_padding_bytes = 12;
                    },
                UniformType::Vec3Array(num_elements) =>
                    {
                        uniform_buffer_size += padding_required(uniform_buffer_size, alignment_mat4x4_float);

                        let entity_id = ecs.create_entity();
                        ecs.write_component::<UniformVec3Array>(entity_id, UniformVec3Array(vec![vec3(0.0, 0.0, 0.0); num_elements as usize]));

                        let expected_source_info = ExpectedUniformData
                        {
                            type_id: TypeId::of::<TVec3<f32>>(),
                            num_elements
                        };

                        uniform_type_ids.insert(uniform.name.clone(), expected_source_info);
                        uniform_entities.insert(uniform.name.clone(), entity_id);

                        sub_padding_bytes = 4;
                    },
                UniformType::Vec4Array(num_elements) =>
                    {
                        uniform_buffer_size += padding_required(uniform_buffer_size, alignment_mat4x4_float);

                        let entity_id = ecs.create_entity();
                        ecs.write_component::<UniformVec4Array>(entity_id, UniformVec4Array(vec![vec4(0.0, 0.0, 0.0, 0.0); num_elements as usize]));

                        let expected_source_info = ExpectedUniformData
                        {
                            type_id: TypeId::of::<TVec4<f32>>(),
                            num_elements
                        };

                        uniform_type_ids.insert(uniform.name.clone(), expected_source_info);
                        uniform_entities.insert(uniform.name.clone(), entity_id);

                        sub_padding_bytes = 0;
                    },
                UniformType::Mat4Array(num_elements) =>
                    {
                        uniform_buffer_size += padding_required(uniform_buffer_size, alignment_mat4x4_float);

                        let entity_id = ecs.create_entity();
                        ecs.write_component::<UniformMat4Array>(entity_id, UniformMat4Array(vec![TMat4::default(); num_elements as usize]));

                        let expected_source_info = ExpectedUniformData
                        {
                            type_id: TypeId::of::<TMat4<f32>>(),
                            num_elements
                        };

                        uniform_type_ids.insert(uniform.name.clone(), expected_source_info);
                        uniform_entities.insert(uniform.name.clone(), entity_id);

                        sub_padding_bytes = 0;
                    }
                UniformType::Vec4 =>
                    {
                        uniform_buffer_size += padding_required(uniform_buffer_size, alignment_mat4x4_float);

                        let entity_id = ecs.create_entity();
                        ecs.write_component::<UniformVec4>(entity_id, UniformVec4(vec4(0.0, 0.0, 0.0, 0.0)));

                        let expected_source_info = ExpectedUniformData
                        {
                            type_id: TypeId::of::<TVec4<f32>>(),
                            num_elements: 1
                        };

                        uniform_type_ids.insert(uniform.name.clone(), expected_source_info);
                        uniform_entities.insert(uniform.name.clone(), entity_id);

                        sub_padding_bytes = 0;
                    }
                UniformType::Float =>
                    {
                        uniform_buffer_size += padding_required(uniform_buffer_size, alignment_scalar);

                        let entity_id = ecs.create_entity();
                        ecs.write_component::<UniformFloat>(entity_id, UniformFloat(0.0));

                        let expected_source_info = ExpectedUniformData
                        {
                            type_id: TypeId::of::<f32>(),
                            num_elements: 1
                        };

                        uniform_type_ids.insert(uniform.name.clone(), expected_source_info);
                        uniform_entities.insert(uniform.name.clone(), entity_id);

                        sub_padding_bytes = 0;
                    }
            }

            // Keep track of where to write the uniform into the array; this information will be searched
            // using the uniform array
            let uniform_data_location = UniformDataLocation
            {
                mapped_buffer_index: mapped_buffers.len(),
                offset_bytes: uniform_buffer_size as isize,
                sub_padding_bytes
            };
            // All uniforms should have a unique name
            assert!(uniform_location_map.insert(uniform.name.clone(), uniform_data_location).is_none());

            // Now actually reserve space for the uniform in the array
            uniform_buffer_size += size_uniform;
        }

        // The type safety for writing to the buffer will be provided by searching the type_id map,
        // rather than keeping that information in the buffer itself
        let mapped_buffer = MappedBuffer::new(uniform_buffer_size as isize, BufferType::UniformBufferArray(mapped_buffers.len() as u32), uniform_block.number_buffers as usize);
        mapped_buffers.push(mapped_buffer);
    }

    UniformResources
    {
        mapped_buffers,
        uniform_location_map,
        uniform_type_ids,
        uniform_entities,
        ecs
    }
}

/// Calculates how many bytes o padding are needed to achieve correct alignment of the next uniform
///
/// `number_to_round` - the number that is being rounded to a multiple
/// `multiple` - the multiple to round to
fn padding_required(number_to_round: usize, multiple: usize) -> usize
{
    if multiple == 0
    {
        return number_to_round;
    }

    let remainder = number_to_round % multiple;
    if remainder == 0
    {
        return remainder;
    }

    (number_to_round + multiple - remainder) - number_to_round
}
