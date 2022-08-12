use nalgebra_glm::{TVec3, TVec4};
use serde::{Serialize, Deserialize};
use crate::exports::logic_components::RenderSystemIndex;
use crate::exports::rendering::LevelOfView;
use crate::world::bounding_volumes::aabb::StaticAABB;

/// Uniquely represents a model that was uploaded to a render system
// Maximum number of model IDs shared across all render system
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModelId
{
    pub model_index: u32, // bits [25, 31] are reserved for the level of view index
pub render_system_index: RenderSystemIndex,
}

pub const NUMBER_MODEL_LEVEL_OF_VIEWS: u32 = 8;

impl ModelId
{
    /// Creates a new model ID
    ///
    /// `model_index` - the model index that is local to a render system
    /// `render_system_index` - the render system index that the model was uploaded into
    pub fn new(model_index: u32, render_system_index: RenderSystemIndex) -> ModelId
    {
        ModelId{ model_index, render_system_index }
    }

    /// Adjusts the model ID to return the effective model ID when taking into account what
    /// level of view a particular instance of a model should be rendered at
    pub fn level_of_view_adjusted_model_index(mut id: ModelId, distance: f32, level_of_views: &Vec<LevelOfView>) -> ModelId
    {
        return match level_of_views.iter().position(|x| x.min_distance <= distance && distance <= x.max_distance)
        {
            Some(i) =>
                {
                    debug_assert!( (i as u32) < NUMBER_MODEL_LEVEL_OF_VIEWS, "Invalid level of view index {}", i);

                    ModelId::apply_level_of_view(&mut id.model_index, i as u32);
                    id
                },
            None =>
                {
                    eprintln!("Invalid distance ({}) specified for level of views: {:?}", distance, level_of_views);
                    ModelId::apply_level_of_view(&mut id.model_index, NUMBER_MODEL_LEVEL_OF_VIEWS as u32 - 1);
                    id
                }
        }
    }

    /// Modifies the model ID according to the level of view index
    ///
    /// `id` - the model ID to modify
    /// `level_of_view_index` - the level of view index to use to modify the DD
    pub fn apply_level_of_view(id: &mut u32, level_of_view_index: u32)
    {
        // There are 8 possible level of views, which corresponds to an index of max 7
        *id |= level_of_view_index.min(NUMBER_MODEL_LEVEL_OF_VIEWS - 1) << 25;
    }
}

/// Holds rendering information used to render the model as well as interact with it logically
pub struct ModelInformation
{
    pub geometry: ModelGeometry,
    pub aabb: OriginalAABB,
    pub instance_count: u32,
}

/// Stores the location of a texture within a texture array
#[derive(Clone, Serialize, Deserialize)]
pub struct TextureLocation
{
    data: [u32; 4]
}

const DIFFUSE_INDEX: u128 = 0;
const DISSOLVE_INDEX: u128 = 1;
const NORMAL_INDEX: u128 = 2;
const SHININESS_INDEX: u128 = 3;
const SPECULAR_INDEX: u128 = 4;

const SIZE_TEXTURE_BITS: u128 = 16;
const SIZE_TEXTURE_INDEX_OFFSET: u128 = 10;

/// This macro generates functions to upload locations of a texture within a texture array
macro_rules! texture_implement
{
    ($fn_name: tt, $texture_type: tt) =>
    {
        pub fn $fn_name(&mut self, array_index: usize, offset_index: i32)
        {
           self.clear_array_index($texture_type);
           self.clear_index_offset($texture_type);

           unsafe { *(self.data.as_mut_ptr() as *mut u128) |= (array_index as u128) << $texture_type * SIZE_TEXTURE_BITS  + SIZE_TEXTURE_INDEX_OFFSET; }
           unsafe { *(self.data.as_mut_ptr() as *mut u128) |= (offset_index as u128) << $texture_type * SIZE_TEXTURE_BITS; }
        }
    };
}

impl TextureLocation
{
    /// Creates a new TextureLocation structure that automatically has the texture types point to
    /// the error texture array
    pub fn place_holder() -> TextureLocation
    {
        let mut texture_location = TextureLocation { data: [0; 4] };

        // By default, the textures used by a model will be from the error texture array.
        // When a model is loaded and the required textures are loaded, then the appropriate indexes will be updated.
        // If a required texture is not loaded and that type of texture (such as specular) is used in the shaders,
        // then the error texture will be used to give a visual indication of a problem occurring with loading textures
        texture_location.write_diffuse(0, 0);
        texture_location.write_dissolve(0, 1);
        texture_location.write_normal(0, 2);
        texture_location.write_shininess(0, 3);
        texture_location.write_specular(0, 4);
        texture_location
    }

    texture_implement!(write_diffuse, DIFFUSE_INDEX);
    texture_implement!(write_dissolve, DISSOLVE_INDEX);
    texture_implement!(write_normal, NORMAL_INDEX);
    texture_implement!(write_shininess, SHININESS_INDEX);
    texture_implement!(write_specular, SPECULAR_INDEX);

    /// Resets the array index of a texture type to 0, allowing future bitwise operations to write
    /// a new array index to be correct. This called only internally, in the write* functions implemented
    /// by the texture_implement macro
    ///
    /// `array_offset` - the offset for the type of texture. See texture type constants above
    fn clear_array_index(&mut self, array_offset: u128)
    {
        let clear_pattern = 0xFC00 as u128;
        unsafe{ *(self.data.as_mut_ptr() as *mut u128) &= !(clear_pattern << array_offset * SIZE_TEXTURE_BITS) }
    }

    /// Resets the index offset of a texture type to 0, allowing future bitwise operations to write
    /// a new index offset to be correctThis called only internally, in the write* functions implemented
    /// by the texture_implement macro
    ///
    /// `index_offset` - the offset for the type of texture. See texture type constants above
    fn clear_index_offset(&mut self, index_offset: u128)
    {
        let clear_pattern = 0x3FF;
        unsafe{ *(self.data.as_mut_ptr() as *mut u128) &= !(clear_pattern << index_offset * SIZE_TEXTURE_BITS) }
    }
}

/// Rendering information to render a mesh
#[derive(Clone, Serialize, Deserialize)]
pub struct MeshGeometry
{
    pub vertices: Vec<TVec3<f32>>,
    pub indices: Vec<u32>,
    pub normals: Vec<TVec3<f32>>,
    pub texture_coords: Vec<TVec4<f32>>,
    pub texture_location: Vec<TextureLocation>,
}

/// Collection of mesh rendering information to render a model
#[derive(Clone, Serialize, Deserialize)]
pub struct ModelGeometry
{
    pub meshes: Vec<MeshGeometry>,
}

/// The bounding volume of the model when it is centred at the origin
#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct OriginalAABB
{
    pub aabb: StaticAABB,
}

#[cfg(test)]
mod tests
{
    use crate::models::model_definitions::{TextureLocation, DIFFUSE_INDEX, DISSOLVE_INDEX, NORMAL_INDEX, SHININESS_INDEX, SPECULAR_INDEX};

    /// Finds the array index and index offset for one of the TextureLocation's array indexes.
    /// The returned values are (current_array_index, current_index_offset, other_array_index, other_index_offset).
    /// Current refers to the texture type passed in, for example, Diffuse
    ///
    /// `array_value` - integer holding two array offsets and two index offsets
    /// `texture_value` - the index of the texture type
    fn unpack_texture(array_value: u32, texture_value: u128) -> (u32, u32, u32, u32)
    {
        // This even/odd check is done so that the first two elements always refer to the current
        // texture type
        if texture_value % 2 == 0
        {
            (
                (array_value & 0xFC00) >> 10,
                array_value & 0x3FF,
                array_value >> 26,
                (array_value >> 16) & 0x3FF
            )
        }
        else
        {
            (
                array_value >> 26,
                (array_value >> 16) & 0x3FF,
                (array_value & 0xFC00) >> 10,
                array_value & 0x3FF,
            )
        }
    }

    /// Check the array indexes and index offsets for the first location array index as created
    /// by the placeholder function
    ///
    /// `texture_location` - the TextureLocation instance holding indexes to check
    fn check_first_default_index_value(texture_location: &TextureLocation)
    {
        // Check specular
        let (array_index, index_offset, _, _) = unpack_texture(texture_location.data[0], DIFFUSE_INDEX);
        assert_eq!(array_index, 0);
        assert_eq!(index_offset, 0);

        // Check shininess
        let (array_index, index_offset, _, _) = unpack_texture(texture_location.data[0], DISSOLVE_INDEX);
        assert_eq!(array_index, 0);
        assert_eq!(index_offset, 1);
    }

    /// Check the array indexes and index offsets for the second location array index as created
    /// by the placeholder function
    ///
    /// `texture_location` - the TextureLocation instance holding indexes to check
    fn check_second_default_index_value(texture_location: &TextureLocation)
    {
        // Check normal
        let (array_index, index_offset, _, _) = unpack_texture(texture_location.data[1], NORMAL_INDEX);
        assert_eq!(array_index, 0);
        assert_eq!(index_offset, 2);

        // Check dissolve
        let (array_index, index_offset, _, _) = unpack_texture(texture_location.data[1], SHININESS_INDEX);
        assert_eq!(array_index, 0);
        assert_eq!(index_offset, 3);
    }

    /// Check the array indexes and index offsets for the third location array index as created
    /// by the placeholder function
    ///
    /// `texture_location` - the TextureLocation instance holding indexes to check
    fn check_third_default_index_value(texture_location: &TextureLocation)
    {
        // Check diffuse
        let (array_index, index_offset, _, _) = unpack_texture(texture_location.data[2], SPECULAR_INDEX);
        assert_eq!(array_index, 0);
        assert_eq!(index_offset, 4);
    }

    #[test]
    fn pack_unpack_diffuse_texture()
    {
        let mut texture_location = TextureLocation::place_holder();
        texture_location.write_diffuse(5, 567);

        // Check that writing to diffuse part of the TextureLocation does not affect other
        // array indexes
        check_second_default_index_value(&texture_location);
        check_third_default_index_value(&texture_location);

        let (array_index, index_offset, other_array, other_index) =
            unpack_texture(texture_location.data[0],DIFFUSE_INDEX);

        // Check that diffuse texture information was written correctly and other texture type
        // information held in same array index is not affected
        assert_eq!(array_index, 5);
        assert_eq!(index_offset, 567);
        assert_eq!(other_array, 0);
        assert_eq!(other_index, 1);
    }

    #[test]
    fn pack_unpack_dissolve_texture()
    {
        let mut texture_location = TextureLocation::place_holder();
        texture_location.write_dissolve(43, 12);

        check_second_default_index_value(&texture_location);
        check_third_default_index_value(&texture_location);

        let (array_index, index_offset, other_array, other_index) =
            unpack_texture(texture_location.data[0],DISSOLVE_INDEX);

        assert_eq!(array_index, 43);
        assert_eq!(index_offset, 12);
        assert_eq!(other_array, 0);
        assert_eq!(other_index, 0);
    }

    #[test]
    fn pack_unpack_normal_texture()
    {
        let mut texture_location = TextureLocation::place_holder();
        texture_location.write_normal(1, 879);

        check_first_default_index_value(&texture_location);
        check_third_default_index_value(&texture_location);

        let (array_index, index_offset, other_array, other_index)
            = unpack_texture(texture_location.data[1],NORMAL_INDEX);

        assert_eq!(array_index, 1);
        assert_eq!(index_offset, 879);
        assert_eq!(other_array, 0);
        assert_eq!(other_index, 3);
    }

    #[test]
    fn pack_unpack_shininess_texture()
    {
        let mut texture_location = TextureLocation::place_holder();
        texture_location.write_shininess(0, 1);

        check_first_default_index_value(&texture_location);
        check_third_default_index_value(&texture_location);

        let (array_index, index_offset, other_array, other_index)
            = unpack_texture(texture_location.data[1],SHININESS_INDEX);

        assert_eq!(array_index, 0);
        assert_eq!(index_offset, 1);
        assert_eq!(other_array, 0);
        assert_eq!(other_index, 2);
    }

    #[test]
    fn pack_unpack_specular_texture()
    {
        let mut texture_location = TextureLocation::place_holder();
        texture_location.write_specular(34, 5);

        check_first_default_index_value(&texture_location);
        check_second_default_index_value(&texture_location);

        let (array_index, index_offset, other_array, other_index) =
            unpack_texture(texture_location.data[2],SPECULAR_INDEX);

        assert_eq!(array_index, 34);
        assert_eq!(index_offset, 5);
        assert_eq!(other_array, 0);
        assert_eq!(other_index, 0);
    }
}