use std::mem::size_of;
use serde::{Serialize, Deserialize};
use crate::exports::load_models::{MaxNumLights, UserLoadSkyBoxModels};
use crate::exports::logic_components::RenderSystemIndex;
use crate::exports::movement_components::TransformationMatrix;
use crate::exports::rendering::LevelOfView;
use crate::helper_things::environment::{get_asset_folder, get_generated_shaders_folder};
use crate::models::model_definitions::MeshGeometry;
use crate::objects::ecs::ECS;
use crate::objects::entity_id::EntityId;
use crate::render_components::mapped_buffer::{BufferWriteInfo, MappedBuffer};
use crate::render_system::builder::{MaxLightConstraints, RenderSystemBuilder};
use crate::render_system::render_system::{InstancedLayoutWriteFunction, RenderSystem};
use crate::render_system::system_information::*;
use crate::specify_model_geometry_layouts;
use crate::specify_type_ids;

pub const DEFAULT_RENDER_SYSTEM: RenderSystemIndex = RenderSystemIndex{ index: 0};

specify_model_geometry_layouts!(model_layout_update_fn,
                                0, vertices,
                                1, texture_coords,
                                2, texture_location,
                                3, normals);

specify_type_ids!(instance_layout_fn,
                  4, TransformationMatrix
                   );

pub fn create_default_render_system(draw_function: DrawFunction, light_draw_function: DrawFunction,
                                    transparency_draw_function: DrawFunction,
                                    instance_layout_update_fn: InstancedLayoutWriteFunction,
                                    level_of_views: Vec<LevelOfView>, window_resolution: (i32, i32),
                                    sky_boxes: Vec<UserLoadSkyBoxModels>,
                                    max_lights: MaxNumLights,
                                    no_light_source_cutoff: f32,
                                    default_diffuse_factor: f32) -> RenderSystem
{
    // TODO: Why does a vec3 variable in uniform block that writes to an out variable not work.
    // TODO: Tested with a vec3 variable that changes skybox brightness

    let mut render_system = RenderSystemBuilder::new()
        .with_constants(vec!
        [
            Constant::new(ConstantValue::UInt(6), "NUMBER_SHADOW_MAPS", vec![ConstantLocation::VertexShader, ConstantLocation::FragmentShader])
        ])
        .with_vertex_shader(VertexShaderInformation
        {
            write_generated_shader: Some(get_generated_shaders_folder().join("first_pass_vertex.glsl").to_str().unwrap().to_string()),
            glsl_version: GLSLVersion::Core430,
            shader_source: get_asset_folder().join("shaders/first_pass_vertex.glsl"),
            instance_layout_update_fn: Some(instance_layout_update_fn),
            model_layout_update_fn,
            indice_buffers: Some(IndiceInformation::new(1, 103100)),
            textures: vec![],
            cubemaps: vec![],
            uniforms: vec!
            [
                UniformBlock::new("Matrices", 4, vec!
                [
                    Uniform::new("projectionMatrix", UniformType::Mat4x4Float),
                    Uniform::new("viewMatrix", UniformType::Mat4x4Float),
                    Uniform::new("cameraLocation", UniformType::Vec3),
                    Uniform::new("renderingSkybox", UniformType::Int),
                    Uniform::new("drawOutline", UniformType::UInt),
                    Uniform::new("lightSource", UniformType::UInt),
                    Uniform::new("renderingLightSource", UniformType::UInt),
                ]),

                UniformBlock::new("LightMatrices", 4, vec!
                [
                    Uniform::new("lightMatrices", UniformType::Mat4Array(6)),
                    Uniform::new("lightViewMatrices", UniformType::Mat4Array(6)),
                    Uniform::new("numberLightMatrices", UniformType::UInt),
                ])
            ],
            layout_info: vec!
            [
                LayoutInformation::new(LayoutType::Vec3Float, LayoutInstance::Divisor0(1, 1_5000_000), LayoutUse::PerModel, "aPos"),
                LayoutInformation::new(LayoutType::Vec4Float, LayoutInstance::Divisor0(1, 2_0000_000), LayoutUse::PerModel, "texCoords"),
                LayoutInformation::new(LayoutType::Vec4Uint, LayoutInstance::Divisor0(1, 2_0000_000), LayoutUse::PerModel, "layers"),
                LayoutInformation::new(LayoutType::Vec3Float, LayoutInstance::Divisor0(1, 1_5000000), LayoutUse::PerModel, "normal"),
                LayoutInformation::new(LayoutType::Mat4x4Float, LayoutInstance::Divisor1(1, 1_520_4000), LayoutUse::PerInstance, "translation"),
            ],
            out_variables: vec!
            [
                OutVariables::new(SharedVariableType::Int, "useSkyboxTexture", true, vec![SharedTarget::FragmentShader]),
                OutVariables::new(SharedVariableType::Vec3, "skyBoxTexCoords", false, vec![SharedTarget::FragmentShader]),
                OutVariables::new(SharedVariableType::Vec3, "fragPosition", false, vec![SharedTarget::FragmentShader]),
                OutVariables::new(SharedVariableType::Vec3, "normalizedVertexNormal", false, vec![SharedTarget::FragmentShader]),
                OutVariables::new(SharedVariableType::Vec3, "cameraPosition", false, vec![SharedTarget::FragmentShader]),
                OutVariables::new(SharedVariableType::Vec4Array(6), "lightFragPos", false, vec![SharedTarget::FragmentShader]),
                OutVariables::new(SharedVariableType::UVec4, "textureLayer", true, vec![SharedTarget::FragmentShader]),
                OutVariables::new(SharedVariableType::Vec4, "textureCoords", false, vec![SharedTarget::FragmentShader]),
                OutVariables::new(SharedVariableType::UInt, "adjustBrightnessLightSource", true, vec![SharedTarget::FragmentShader]),
                OutVariables::new(SharedVariableType::UInt, "drawingLightSource", true, vec![SharedTarget::FragmentShader]),
            ]

        })
        .with_first_pass_fragment_shader(FragmentShaderInformation
        {
            layouts: vec!
            [
                FragLayoutInformation::new(LayoutType::Vec3Float, TextureFormat::RGBA16F, window_resolution, "gPosition"),
                FragLayoutInformation::new(LayoutType::Vec3Float, TextureFormat::RGBA16F, window_resolution,"gNormal"),
                FragLayoutInformation::new(LayoutType::Vec4Float, TextureFormat::RGBA, window_resolution,"gAlbedoSpec"),
                FragLayoutInformation::new(LayoutType::Vec4Float, TextureFormat::RGBA16F, window_resolution,"gLightPosition")
            ],
            uniforms: vec![],
            out_variables: vec![],
            write_generated_shader: Some(get_generated_shaders_folder().join("first_pass_frag.glsl").to_str().unwrap().to_string()),
            glsl_version: GLSLVersion::Core430,
            shader_source: get_asset_folder().join("shaders/first_pass_frag.glsl"),
            textures: vec!
            [
                TextureInformation
                {
                    sampler_name: "textureArray".to_string(),
                    number_mipmaps: 5,
                    format: TextureFormat::RGBA,
                    min_filter_options: MinFilterOptions::Linear,
                    mag_filter_options: MagFilterOptions::Linear,
                    wrap_s: TextureWrap::MirroredRepeat,
                    wrap_t: TextureWrap::MirroredRepeat,
                    width: 2560,
                    height: 1440,
                    number_textures: 5,
                    border_color: None
                },
                TextureInformation
                {
                    sampler_name: "solidColour".to_string(),
                    number_mipmaps: 1,
                    format: TextureFormat::RGBA,
                    min_filter_options: MinFilterOptions::Nearest,
                    mag_filter_options: MagFilterOptions::Nearest,
                    wrap_s: TextureWrap::ClampToEdge,
                    wrap_t: TextureWrap::ClampToEdge,
                    width: 1,
                    height: 1,
                    number_textures: 25,
                    border_color: None
                }
            ],
            cubemaps: vec!
            [
                CubeMapInitInfo::new("skyBox")
            ],
            include_shadow_maps: false,
            include_error_textures: true,
        })
        .with_second_pass_vertex_shader()
        .with_second_pass_fragment_shader(FragmentShaderInformation
        {
            layouts: vec![],
            out_variables: vec![OutVariables::new(SharedVariableType::Vec4, "FragColor", false, vec![])],
            write_generated_shader: Some(get_generated_shaders_folder().join("second_pass_frag.glsl").to_str().unwrap().to_string()),
            include_error_textures: false,
            include_shadow_maps: true,
            glsl_version: GLSLVersion::Core430,
            shader_source: get_asset_folder().join("shaders/second_pass_frag.glsl"),
            uniforms: vec!
            [
                UniformBlock::new("LightSources", 4, vec!
                [
                    Uniform::new("anyLightSourceVisible", UniformType::UInt),
                    Uniform::new("directionLightDirection", UniformType::Vec3Array(max_lights.directional)),
                    Uniform::new("directionLightDiffuseColour", UniformType::Vec3Array(max_lights.directional)),
                    Uniform::new("directionLightSpecularColour", UniformType::Vec3Array(max_lights.directional)),
                    Uniform::new("directionLightAmbientColour", UniformType::Vec4Array(max_lights.directional)),
                    Uniform::new("numberDirectionLights", UniformType::UInt),

                    Uniform::new("spotLightPosition", UniformType::Vec3Array(max_lights.spot)),
                    Uniform::new("spotLightDiffuseColour", UniformType::Vec3Array(max_lights.spot)),
                    Uniform::new("spotLightSpecularColour", UniformType::Vec3Array(max_lights.spot)),
                    Uniform::new("spotLightAmbientColour", UniformType::Vec4Array(max_lights.spot)),
                    Uniform::new("spotLightLinearCoefficient", UniformType::FloatArray(max_lights.spot)),
                    Uniform::new("spotLightQuadraticCoefficient", UniformType::FloatArray(max_lights.spot)),
                    Uniform::new("spotLightRadius", UniformType::FloatArray(max_lights.spot)),
                    Uniform::new("numberSpotLights", UniformType::UInt),

                    Uniform::new("pointLightPosition", UniformType::Vec3Array(max_lights.point)),
                    Uniform::new("pointLightDirection", UniformType::Vec3Array(max_lights.point)),
                    Uniform::new("pointLightDiffuseColour", UniformType::Vec3Array(max_lights.point)),
                    Uniform::new("pointLightSpecularColour", UniformType::Vec3Array(max_lights.point)),
                    Uniform::new("pointLightAmbientColour", UniformType::Vec4Array(max_lights.point)),
                    Uniform::new("pointLightLinearCoefficient", UniformType::FloatArray(max_lights.point)),
                    Uniform::new("pointLightQuadraticCoefficient", UniformType::FloatArray(max_lights.point)),
                    Uniform::new("cutOff", UniformType::FloatArray(max_lights.point)),
                    Uniform::new("outerCutoff", UniformType::FloatArray(max_lights.point)),
                    Uniform::new("numberPointLights", UniformType::UInt),

                    Uniform::new("cameraPosition", UniformType::Vec3),
                    Uniform::new("fragDrawOutline", UniformType::UInt),
                    Uniform::new("noLightSourceCutoff", UniformType::Float),
                    Uniform::new("defaultDiffuseFactor", UniformType::Float),
                    Uniform::new("renderSkybox", UniformType::UInt)
                ]),

                UniformBlock::new("LightIndexes", 4, vec!
                [
                    Uniform::new("lightIndexes", UniformType::UIntArray(6)),
                    Uniform::new("numberLightIndexes", UniformType::UInt)
                ]),
            ],

            textures: vec![],
            cubemaps: vec![],
        })
        .with_draw_functions(draw_function, light_draw_function, transparency_draw_function)
        .with_level_of_views(level_of_views)
        .with_accessible_fbos(vec![])
        .apply_nearby_lights()
        .with_light_constraints(MaxLightConstraints::Constraints(max_lights))
        .with_no_light_diffuse_param(no_light_source_cutoff, default_diffuse_factor)
        .build();

    for x in sky_boxes
    {
        render_system.load_cubemap(x.sky_box_name, x.textures);
    }

    render_system.bind_cubemap("skyBox");

    render_system.register_uniform_type_ecs::<UseSkyBox>();
    let entity_id = render_system.add_uniform_entities("useSkyBox");
    let render_sky_box = UseSkyBox{ use_sky_box: 0 };
    render_system.write_uniform_value(entity_id, render_sky_box);

    render_system
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct UseSkyBox
{
    pub(crate) use_sky_box: i32,
}

pub const NUMBER_DEFAULT_LEVEL_VIEWS: usize = 5;
pub fn create_level_of_views(render_distance: f32) -> Vec<LevelOfView>
{
    let first_view_distance = render_distance * 0.10;
    let second_view_distance = render_distance * 0.15 + first_view_distance;
    let third_view_distance = render_distance * 0.20 + second_view_distance;
    let fourth_view_distance = render_distance * 0.25 + third_view_distance;
    let fifth_view_distance = render_distance * 0.30 + fourth_view_distance;

    vec!
    [
        LevelOfView{ min_distance: 0.0, max_distance: first_view_distance },
        LevelOfView{ min_distance: first_view_distance, max_distance: second_view_distance },
        LevelOfView{ min_distance: second_view_distance, max_distance: third_view_distance },
        LevelOfView{ min_distance: third_view_distance, max_distance: fourth_view_distance },
        LevelOfView{ min_distance: fourth_view_distance, max_distance: fifth_view_distance }
    ]
}