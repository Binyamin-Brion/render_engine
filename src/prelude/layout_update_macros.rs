#[macro_export]
macro_rules! specify_type_ids
{
    ($function_name: tt, $($index: expr, $associated_type: ident),+) =>
    {
        pub fn $function_name(layout_index: u32, ecs: &ECS, buffer_write_destination: &mut Vec<u8>, entity_index: EntityId)
        {
            match layout_index
            {
                $(
                    $index =>
                    {
                        unsafe
                        {
                            let write_index = buffer_write_destination.len() as isize;
                            for _ in 0..size_of::<$associated_type>()
                            {
                                buffer_write_destination.push(0);
                            }
                            *(buffer_write_destination.as_ptr().offset(write_index) as *mut $associated_type) =
                             ecs.get_copy::<$associated_type>(entity_index).unwrap();
                        }
                    },
                )+
                _ => {}
            }
        }
    };
}

#[macro_export]
macro_rules! specify_model_geometry_layouts
{
      ($function_name: tt, $($index: expr, $param_name: tt),*) =>
      {
          #[allow(unreachable_code)]
          fn $function_name(layout_index: u32, _model_geometry: &MeshGeometry, _buffer_write_destination: BufferWriteInfo, _buffer_offset_bytes: isize) -> isize
          {
              return match layout_index
              {
                  $(
                        $index =>
                        {
                            MappedBuffer::write_data_serialized(_buffer_write_destination, &_model_geometry.$param_name, _buffer_offset_bytes, true)
                        },
                  )*
                  _ => unreachable!()
              }
          }
      }
}