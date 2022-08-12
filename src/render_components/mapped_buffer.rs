use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::{copy_nonoverlapping, null};
use gl::types::GLsync;

/// A buffer that supports updating data within itself without causing GPU stalls
pub struct MappedBuffer
{
    buffer: Vec<u32>,
    fence: Vec<GLsync>,
    ptr: Vec<*mut c_void>,
    pub current_instance_buffer_index: usize,
    buffer_type: BufferType,
    number_buffers: usize,
    pub size_buffer_bytes: isize,
    is_fence_set: bool,
}

/// Required information to write to a buffer
#[derive(Copy, Clone)]
pub struct BufferWriteInfo
{
    ptr: *mut c_void,
    size_buffer_bytes: isize,
}

pub type BindingPoint = u32;

/// Specifies the binding information for the current buffer
pub struct BindingInformation
{
    binding_point: BindingPoint,
    offset: isize,
    stride: i32,
}

/// Specifies if the current buffer is to be used for indices
// This is needed because if the buffer is for indices, then an explicit bind to the ELEMENT_ARRAY_BUFFER
// target is required for the VAO to use indices
pub enum BufferType
{
    IndiceArray,
    NonIndiceArray(Vec<BindingInformation>),
    UniformBufferArray(BindingPoint),
}

/// Represents possible errors that can occur when waiting for a buffer to be available for writing
/// new data
#[derive(Debug)]
pub enum WaitResult
{
    Timeout,
    UnknownFailure,
}

// On some GPUs, using coherent buffers leads to artifacts
const USE_COHERENT_BUFFERS: bool = true;

impl MappedBuffer
{
    /// Creates a new mapped buffer with the given size in bytes and the given type
    ///
    /// `size_buffer_bytes` - the size of this buffer in bytes. Note due to the implementation, the actual
    ///                       vRAM used by this buffer will be greater than the size passed in, The buffer
    ///                       can still only hold the amount passed in.
    /// `buffer_type` - whether or not this buffer is for indices
    /// `number_buffers` - the number of buffers to use in a round-robin fashion to prevent stalling
    pub fn new(size_buffer_bytes: isize, buffer_type: BufferType, number_buffers: usize) -> MappedBuffer
    {
        let mut buffer = Vec::with_capacity(number_buffers);
        let mut ptr =  Vec::with_capacity(number_buffers);
        let mut fence =  Vec::with_capacity(number_buffers);

        let buffer_bitmap = if USE_COHERENT_BUFFERS
        {
            gl::MAP_WRITE_BIT | gl::MAP_PERSISTENT_BIT | gl::MAP_COHERENT_BIT
        }
        else
        {
            gl::MAP_WRITE_BIT | gl::MAP_PERSISTENT_BIT
        };

        let ptr_bitmap = if USE_COHERENT_BUFFERS
        {
            gl::MAP_WRITE_BIT | gl::MAP_PERSISTENT_BIT | gl::MAP_UNSYNCHRONIZED_BIT | gl::MAP_COHERENT_BIT
        }
        else
        {
            gl::MAP_WRITE_BIT | gl::MAP_PERSISTENT_BIT | gl::MAP_UNSYNCHRONIZED_BIT | gl::MAP_FLUSH_EXPLICIT_BIT
        };

        unsafe
            {
                for _ in 0..number_buffers
                {
                    let mut new_buffer: u32 = 0;

                    gl::CreateBuffers(1, &mut new_buffer);
                    gl::NamedBufferStorage(new_buffer, size_buffer_bytes, null(), buffer_bitmap);

                    buffer.push(new_buffer);
                    ptr.push( gl::MapNamedBufferRange(new_buffer, 0, size_buffer_bytes, ptr_bitmap) );
                    fence.push(gl::FenceSync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0) );

                }
            }

        MappedBuffer{ buffer, ptr, fence, current_instance_buffer_index: 0, buffer_type, number_buffers, size_buffer_bytes, is_fence_set: true }
    }

    /// Waits for the next buffer scheduled to be written to, and will block the calling thread until
    /// that buffer is ready to be written to.
    ///
    /// The returned type, if successful, is a tuple of the pointer to the buffer, and an enum specifying
    /// what source structure to be used for uploading data (if buffer is not being used for uniforms).
    ///
    /// This tuple is passed into write_data_vertex_layout, if uniforms are not being written to
    ///
    /// `timeout` - how long to wait (in a single wait query) until a backing buffer is free to use
    pub fn wait_for_next_free_buffer(&mut self, timeout: u64) -> Result<BufferWriteInfo, WaitResult>
    {
        if !self.is_fence_set
        {
            return Ok(BufferWriteInfo{ ptr: self.ptr[self.current_instance_buffer_index], size_buffer_bytes: self.size_buffer_bytes} );
        }

        // Check if buffer is free without waiting or flushing, so there is no penalty for being fast
        let mut fence_result = unsafe{ gl::ClientWaitSync(self.fence[self.current_instance_buffer_index], 0, 0)};

        if fence_result == gl::TIMEOUT_EXPIRED
        {
            // Buffer is not free yet, try to flush and wait the desired amount
            fence_result = unsafe{ gl::ClientWaitSync(self.fence[self.current_instance_buffer_index], gl::SYNC_FLUSH_COMMANDS_BIT, timeout)};

            if fence_result == gl::TIMEOUT_EXPIRED
            {
                // Buffer is really not free to be updated; have to stall until the buffer is ready
                unsafe { gl::Finish(); }

                // Try again
                fence_result = unsafe{ gl::ClientWaitSync(self.fence[self.current_instance_buffer_index], gl::SYNC_FLUSH_COMMANDS_BIT, timeout)};

                if fence_result == gl::TIMEOUT_EXPIRED
                {
                    // At this point, even after glFinish(), buffer is still not free- this is a serious failure,
                    // no point trying again
                    return Err(WaitResult::Timeout);
                }
            }
        }

        if fence_result == gl::WAIT_FAILED
        {
            return Err(WaitResult::UnknownFailure);
        }

        self.is_fence_set = false;
        Ok(BufferWriteInfo{ ptr: self.ptr[self.current_instance_buffer_index], size_buffer_bytes: self.size_buffer_bytes} )
    }

    /// Write data to the buffer without any type safety checks
    ///
    /// `write_information` - information required to write to a mapped buffer
    /// `data` - the data to write to the buffer
    /// `offset_bytes` - offset in bytes from the start of the buffer to write the provided data to
    pub fn write_data_serialized<T: 'static>(write_information: BufferWriteInfo, data: &[T], offset_count: isize, fail_buffer_small: bool) -> isize
    {
        let size_type = size_of::<T>();
        let bytes_to_write = data.len() * size_type;

        if (bytes_to_write + offset_count as usize) > write_information.size_buffer_bytes as usize
        {
            if fail_buffer_small
            {
                panic!("Attempting to write {} bytes of data into buffer of {} bytes large with byte offset {}", bytes_to_write, write_information.size_buffer_bytes, offset_count);
            }
            else
            {
                eprintln!("Attempting to write {} bytes of data into buffer of {} bytes large with byte offset {}", bytes_to_write, write_information.size_buffer_bytes, offset_count);
            }
        }

        unsafe
            {
                copy_nonoverlapping(data.as_ptr() as *const u8, (write_information.ptr as *mut u8).offset(offset_count), bytes_to_write.min(write_information.size_buffer_bytes as usize));
            }

        (size_type * data.len()) as isize
    }

    /// Write a single piece of data to the buffer without any type safety checks
    ///
    /// `write_information` - information required to write to a mapped buffer
    /// `data` - the data to write to the buffer
    /// `offset_bytes` - offset in bytes from the start of the buffer to write the provided data to
    pub fn write_single_serialized_value<T: 'static>(write_information: BufferWriteInfo, data: T, offset_count: isize, fail_buffer_small: bool) -> isize
    {
        MappedBuffer::write_data_serialized(write_information, &[data], offset_count, fail_buffer_small)
    }

    /// Marks the buffer as finished, meaning all written data is flushed. The number of bytes changed
    /// specifies a range of [0, number_bytes_changed]
    ///
    /// `start_byte_changed` - the start of the range that was modified
    /// `number_bytes_changed` - the number of bytes from the start of the rnage that was changed
    pub fn mark_buffer_updates_finish(&mut self, start_byte_changed: isize, number_bytes_changed: isize)
    {
        if !USE_COHERENT_BUFFERS
        {
            unsafe
                {
                    gl::FlushMappedNamedBufferRange(self.buffer[self.current_instance_buffer_index], start_byte_changed, number_bytes_changed);

                    // Apparently needed according to OpenGL spec for glBufferStorage- still doesn't seem to fix
                    // issue of using flush calls with Nvidia
                    gl::MemoryBarrier(gl::CLIENT_MAPPED_BUFFER_BARRIER_BIT);
                }
        }

        // Previous binding refers to different round robin buffer; need to make sure to use the buffer
        // that was just updated
        self.bind_current_buffer();
    }

    /// Helper function that flushes the entire buffer, negating need to specify a range
    pub fn flush_entire_buffer(&mut self)
    {
        self.mark_buffer_updates_finish(0, self.size_buffer_bytes);
    }

    /// Updates the current buffer to use the given binding information, with the updated backing
    /// buffer now providing the source of inputs
    pub fn bind_current_buffer(&self)
    {
        match self.buffer_type
        {
            BufferType::NonIndiceArray(ref binding_information) =>
                {
                    unsafe
                        {
                            for current_binding_information in binding_information
                            {
                                gl::BindVertexBuffer
                                    (
                                        current_binding_information.binding_point,
                                        self.buffer[self.current_instance_buffer_index],
                                        current_binding_information.offset,
                                        current_binding_information.stride
                                    );
                            }
                        }
                },

            BufferType::IndiceArray => {
                unsafe
                    {
                        gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.buffer[self.current_instance_buffer_index])
                    }
            },
            BufferType::UniformBufferArray(binding_point) =>
                {
                    unsafe
                        {
                            gl::BindBufferBase(gl::UNIFORM_BUFFER, binding_point, self.buffer[self.current_instance_buffer_index])
                        }
                }

        }
    }

    /// Updates the fences for the buffer. This must be called right after the draw operations that use
    /// the buffer
    pub fn set_fence(&mut self)
    {
        unsafe{ gl::DeleteSync(self.fence[self.current_instance_buffer_index]) }
        self.fence[self.current_instance_buffer_index] = unsafe { gl::FenceSync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0) };

        self.current_instance_buffer_index = (self.current_instance_buffer_index + 1) % self.number_buffers;
        self.is_fence_set = true;
    }
}

impl BindingInformation
{
    /// Creates a new structure of binding information
    ///
    /// `binding_point` - the binding point for the MappedBuffer
    /// `offset` - the start of a range within the buffer to use when binding
    /// `stride` - the stride of the buffer
    pub fn new(binding_point: u32, offset: isize, stride: i32) -> BindingInformation
    {
        BindingInformation{ binding_point, offset, stride }
    }
}