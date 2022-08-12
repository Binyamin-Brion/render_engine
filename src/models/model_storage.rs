use std::fmt::Debug;
use std::path::{Path, PathBuf};
use hashbrown::HashMap;
use nalgebra_glm::{TVec4, vec3, vec4};
use crate::exports::logic_components::RenderSystemIndex;
use crate::exports::rendering::LevelOfView;
use crate::flows::render_flow::RenderFlow;
use crate::helper_things::aabb_helper_functions;
use crate::models::model_definitions::{MeshGeometry, ModelGeometry, ModelId, ModelInformation,
                                       OriginalAABB, TextureLocation};
use crate::prelude::default_render_system::NUMBER_DEFAULT_LEVEL_VIEWS;
use crate::render_system::render_system::UploadedTextureLocation;
use crate::world::bounding_volumes::aabb::StaticAABB;

/// Owner of all model banks, effectively holding the models for all of the render system
pub struct ModelBankOwner
{
    name_model_lookup: HashMap<String, ModelId>,
    model_banks: Vec<ModelBank>,
    free_ids: Vec<ModelId>,
    number_models_loaded: usize,
}

/// Holds uploaded models for a render system
pub struct ModelBank
{
    models: HashMap<ModelId, ModelInformation>,
    change_models_number_user_render_system: bool,
    change_models_number_shadow_render_system: bool,
}

/// Information required to register a model with a model bank
pub struct LoadModelInfo<T: Into<String>>
{
    pub model_name: T,
    pub render_system_index: RenderSystemIndex,
    pub location: Vec<PathBuf>,
    pub custom_level_of_view: Option<Vec<LevelOfView>>,
    pub model_texture_dir: PathBuf,
    pub solid_colour_texture: Option<TVec4<u8>>
}

/// This macro uploads different type of textures used by the model into the render system and creates
/// the required data to use that texture in shaders
macro_rules! use_texture_type
{
    ($materials: tt, $render_flow: tt, $render_system_index: tt, $($texture_type: tt, $fn_name: tt),+) =>
    {{

        struct MeshMaterial
        {
            $(
                $texture_type: Option<UploadedTextureLocation>,
            )+
        }

        let mut material_location: HashMap<usize, MeshMaterial> = HashMap::default();
        let mut texture_location = TextureLocation::place_holder();

        for (index, x) in $materials.iter().enumerate()
        {
            $(
                let $texture_type = if x.$texture_type.is_empty()
                {
                    None
                }
                else
                {
                    let uploaded_texture = $render_flow.add_texture(RenderSystemIndex{ index: $render_system_index as usize}, Path::new(&x.$texture_type).to_path_buf());
                    texture_location.$fn_name(uploaded_texture.array_index, uploaded_texture.index_offset);
                    Some(uploaded_texture)
                };
            )+

            material_location.insert(index,
                MeshMaterial
                {
                    $(
                        $texture_type,
                    )+
                }
            );
        }

        (material_location, texture_location)
    }};
}

fn append_texture_dir(texture: &mut String, texture_dir: &PathBuf)
{
    if !texture.is_empty()
    {
        texture.insert_str(0, texture_dir.to_str().unwrap());
    }
}

impl ModelBankOwner
{
    /// Creates a new ModelBankOwner that holds the given number of model banks
    ///
    /// `number_render_systems` - how many render system are in use, and as a result how many model
    ///                          banks are created
    pub fn new(number_render_systems: usize) -> ModelBankOwner
    {
        ModelBankOwner{ name_model_lookup: HashMap::default(), model_banks: (0..number_render_systems).into_iter().map(|_| ModelBank::new()).collect(), number_models_loaded: 0, free_ids: Vec::new() }
    }

    /// Get information about the stored model
    ///
    /// `model_id` - the ID of the model to query
    pub fn get_model_info(&self, model_id: ModelId) -> Option<&ModelInformation>
    {
        self.model_banks[model_id.render_system_index.index].models.get(&model_id)
    }

    fn upload_model_geometry_solid_texture<A: AsRef<Path> + Debug + Clone>(&mut self, location: A, render_system_index: u32, model_id: ModelId, render_flow: &mut RenderFlow, colour: TVec4<u8>)
    {
        let uploaded_texture = render_flow.add_solid_colour_texture(RenderSystemIndex{ index: render_system_index as usize}, colour);
        let mut texture_location = TextureLocation::place_holder();
        texture_location.write_diffuse(uploaded_texture.array_index, uploaded_texture.index_offset);
println!("Loaded: {:?}", location.as_ref());
        let (mut models, _) = tobj::load_obj(location, true).unwrap();
        let mut model_geometry = Vec::new();
        let mut model_aabb = StaticAABB::point_aabb();

        // Load and store all of the rendering information
        for x in models.iter_mut()
        {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            let mut normals = Vec::new();
            let mut texture_coords  = Vec::new();

            indices.append(&mut x.mesh.indices);

            for v in 0..x.mesh.positions.len() / 3
            {
                let vertex = vec3(x.mesh.positions[3 * v], x.mesh.positions[3 * v + 1], x.mesh.positions[3 * v + 2]);
                vertices.push(vertex);
                texture_coords.push(vec4(0.0, 0.0, 0.0, 0.0));
            }

            for n in 0..x.mesh.normals.len() / 3
            {
                let normal = vec3(x.mesh.normals[3 * n], x.mesh.normals[3 * n + 1], x.mesh.normals[3 * n + 2]);
                normals.push(normal);
            }

            // Combine all of the mesh AABB to find the overall bounding volume of the model
            model_aabb = model_aabb.combine_aabb(&aabb_helper_functions::calculate_aabb(&vertices));

            model_geometry.push(MeshGeometry
            {
                texture_location: vec![texture_location.clone(); vertices.len()],
                vertices,
                indices,
                normals,
                texture_coords,
            });

        }

        self.model_banks[render_system_index as usize].add_model(model_id, ModelGeometry{ meshes: model_geometry }, model_aabb);
    }

    /// Upload model geometry and textures to the given render system
    ///
    /// `location` - the location of the asset file that contains the Model rendering information
    /// `render_system_index` - the index of the render system to upload the model to
    /// `model_id` - the ID of the model to upload
    /// `render_flow` - instance of render flow that owns the render systems
    fn upload_model_geometry<A: AsRef<Path> + Debug + Clone>(&mut self, location: A, render_system_index: u32, model_id: ModelId, render_flow: &mut RenderFlow, texture_dir: &PathBuf)
    {
        let (mut models, mut materials) = tobj::load_obj(location, true).unwrap();

        for x in &mut materials
        {
            append_texture_dir(&mut x.ambient_texture, &texture_dir);
            append_texture_dir(&mut x.diffuse_texture, &texture_dir);
            append_texture_dir(&mut x.dissolve_texture, &texture_dir);
            append_texture_dir(&mut x.normal_texture, &texture_dir);
            append_texture_dir(&mut x.shininess_texture, &texture_dir);
            append_texture_dir(&mut x.specular_texture, &texture_dir);
        }

        // Upload the textures to the render system and create the texture locations to index into
        // texture arrays in the shaders
        let (material_location, texture_location) =

            // At time of writing, only diffuse textures are used. To add others, follow same pattern
            // of input to macro as diffuse. For example:  dissolve_texture, write_dissolve
            use_texture_type!(materials, render_flow, render_system_index,
                         diffuse_texture, write_diffuse);

        let mut model_geometry = Vec::new();
        let mut model_aabb = StaticAABB::point_aabb();

        // Load and store all of the rendering information
        for x in models.iter_mut()
        {
            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            let mut normals = Vec::new();
            let mut texture_coords  = Vec::new();

            indices.append(&mut x.mesh.indices);

            for v in 0..x.mesh.positions.len() / 3
            {
                let vertex = vec3(x.mesh.positions[3 * v], x.mesh.positions[3 * v + 1], x.mesh.positions[3 * v + 2]);
                vertices.push(vertex)
            }

            for n in 0..x.mesh.normals.len() / 3
            {
                let normal = vec3(x.mesh.normals[3 * n], x.mesh.normals[3 * n + 1], x.mesh.normals[3 * n + 2]);
                normals.push(normal);
            }

            for t in 0..x.mesh.texcoords.len() / 2
            {
                let coord = vec4(x.mesh.texcoords[2 * t], x.mesh.texcoords[2 * t + 1], 0.0, 0.0);
                texture_coords.push(coord);
            }

            if let Some(material_index) = x.mesh.material_id
            {
                let texture_information = material_location.get(&material_index).unwrap();

                // Write the scaling information for the texture coords when accessing the textures.
                // The information for texture array index and the layer of the texture array is stored
                // separately from the texture coordinates
                for tex_coord in &mut texture_coords
                {
                    match texture_information.diffuse_texture.as_ref()
                    {
                        Some(i) =>
                            {
                                tex_coord[2] = i.scale_x;
                                tex_coord[3] = i.scale_y;
                            },
                        None =>
                            {
                                tex_coord[2] = 1.0;
                                tex_coord[3] = 1.0;
                            }
                    }
                }
            }

            // Combine all of the mesh AABB to find the overall bounding volume of the model
            model_aabb = model_aabb.combine_aabb(&aabb_helper_functions::calculate_aabb(&vertices));

            model_geometry.push(MeshGeometry
            {
                texture_location: vec![texture_location.clone(); vertices.len()],
                vertices,
                indices,
                normals,
                texture_coords,
            });

        }

        self.model_banks[render_system_index as usize].add_model(model_id, ModelGeometry{ meshes: model_geometry }, model_aabb);
    }

    fn get_model_id(&mut self, render_system_index: RenderSystemIndex) -> ModelId
    {
        match self.free_ids.pop()
        {
            Some(i) => i,
            None =>
                {
                    self.number_models_loaded += 1;
                    ModelId::new(self.number_models_loaded as u32, render_system_index)
                }
        }
    }

    pub fn lookup_model(&self, name: &String) -> Option<&ModelId>
    {
        self.name_model_lookup.get(name)
    }

    /// Create a model ID for the given model and upload its rendering information to the desired
    /// render system. After this call, instances of this model can be created
    ///
    /// `model_info` - the model information required to register the model
    /// `render_flow` - owners of all of the render systems
    pub fn register_model<T: Into<String> + Clone>(&mut self, model_info: &LoadModelInfo<T>, render_flow: &mut RenderFlow) -> ModelId
    {
        // Need a model for every level of view
        match model_info.custom_level_of_view
        {
            Some(ref i) => assert_eq!(model_info.location.len(), i.len()),
            None => assert_eq!(NUMBER_DEFAULT_LEVEL_VIEWS, model_info.location.len())
        }

        let base_model_id = self.get_model_id(model_info.render_system_index);

        // Upload all of the rendering geometry for the different level of views
        for x in 0..model_info.location.len()
        {
            let adjusted_model_id =
                {
                    let mut copy_model_id = base_model_id;
                    ModelId::apply_level_of_view(&mut copy_model_id.model_index, x as u32);
                    copy_model_id
                };

            if let Some(colour) = model_info.solid_colour_texture
            {
                self.upload_model_geometry_solid_texture(model_info.location[x].clone(), model_info.render_system_index.index as u32,
                                                         adjusted_model_id, render_flow, colour);
            }
            else
            {
                self.upload_model_geometry(model_info.location[x].clone(), model_info.render_system_index.index as u32,
                                           adjusted_model_id, render_flow, &model_info.model_texture_dir);
            }
        }

        self.name_model_lookup.insert(model_info.model_name.clone().into(), base_model_id);

        base_model_id
    }

    /// Determines if the models contained in the model bank associated with the given render system
    /// needs to be reuploaded
    ///
    /// `render_system_index` - the index of the render system to check if models need to be reuploaded
    pub fn require_reupload_models_user_render_system(&self, render_system_index: usize) -> bool
    {
        let reupload_models = self.model_banks[render_system_index].change_models_number_user_render_system;
        reupload_models
    }

    /// Check's if models need to be reuploaded from any of the model banks. Only the shadow render system
    /// needs to be concerned with checking all of the model banks
    pub fn any_models_changed_shadow_perspective(&self) -> bool
    {
        let mut reupload_models = false;

        for x in  &self.model_banks
        {
            reupload_models |= x.change_models_number_shadow_render_system;
        }

        reupload_models
    }

    /// Clears the flag indicating that models need to be reuploaded for a specific render system
    ///
    /// `render_system_index` - the index of the render system that needs its flag cleared
    pub fn clear_user_render_system_upload_flag(&mut self, render_system_index: usize)
    {
        self.model_banks[render_system_index].change_models_number_user_render_system = false;
    }

    /// Clears the flag indicating that models need to be reuploaded for shadow render system
    pub fn clear_shadow_render_system_upload_flag(&mut self)
    {
        for x in &mut self.model_banks
        {
            x.change_models_number_shadow_render_system = false;
        }
    }

    /// Gets the model bank for a specific render system
    ///
    /// `render_system_index` - the render system that the returned model bank is associated with
    pub fn get_model_bank(&self, render_system_index: RenderSystemIndex) -> &ModelBank
    {
        &self.model_banks[render_system_index.index]
    }

    /// Register instances of the given model
    ///
    /// `model_id` - the ID of the model to register instances
    /// `number_instances` - the number of instances of the provided model to register
    pub fn register_instances(&mut self, model_id: ModelId, number_instances: u32)
    {
        self.model_banks[model_id.render_system_index.index].register_model_instances(model_id, number_instances);
    }

    /// Remove an instance of the model provided
    ///
    /// `model_id` - the ID of the model to have an instance removed
    pub fn remove_instance(&mut self, model_id: ModelId)
    {
        if self.model_banks[model_id.render_system_index.index].remove_instance(model_id)
        {
            self.free_ids.push(model_id);
        }
    }
}

impl ModelBank
{
    /// Creates a new empty model bank
    pub fn new() -> ModelBank
    {
        ModelBank
        {
            models: HashMap::default(),
            change_models_number_user_render_system: false,
            change_models_number_shadow_render_system: false
        }
    }

    /// Register the model with this model bank, making it keep track of its geometry and the number
    /// instances of that model
    ///
    /// `model_id` - the ID of the model to register
    /// `geometry` - the rendering information of the model being added
    /// `aabb` - the surrounding bounding volume of the model being added
    pub fn add_model(&mut self, model_id: ModelId, geometry: ModelGeometry, aabb: StaticAABB)
    {
        let model_information = ModelInformation
        {
            geometry,
            instance_count: 0,
            aabb: OriginalAABB{ aabb }
        };

        // Notify the render systems that new models need to be uploaded
        self.change_models_number_user_render_system = true;
        self.change_models_number_shadow_render_system = true;

        self.models.insert(model_id, model_information);
    }

    /// Add to the instance count of the given model
    ///
    /// `model_id` - the ID of the model whose instance count should be increased
    /// `number_instances` - the number of instances of the given model being created
    pub fn register_model_instances(&mut self, model_id: ModelId, number_instances: u32)
    {
        self.models.get_mut(&model_id).unwrap().instance_count += number_instances as u32;
    }

    /// Decrease the instance count by one for the model being passed into this function
    ///
    /// `model_id` - the ID of the model having an instanced removed
    pub fn remove_instance(&mut self, model_id: ModelId) -> bool
    {
        let model = self.models.get_mut(&model_id).unwrap();
        model.instance_count -= 1;
        if model.instance_count == 0
        {
            self.models.remove(&model_id);
            self.change_models_number_user_render_system = true;
            self.change_models_number_shadow_render_system = true;
            return true;
        }

        return false;
    }

    /// Get all of the models stored in this model bank
    pub fn stored_models(&self) -> &HashMap<ModelId, ModelInformation>
    {
        &self.models
    }
}