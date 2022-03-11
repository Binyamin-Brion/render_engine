use gl;
use gl::types::GLenum;

/// Represents the VAO for a render system.
pub struct VAO
{
    vao: u32
}

impl VAO
{
    /// Creates a new VAO; the vao is not bound after this function
    pub fn new() -> VAO
    {
        let mut vao: u32 = 0;
        unsafe{ gl::GenVertexArrays(1, &mut vao); }
        VAO{ vao }
    }

    /// Binds the vao
    pub fn bind(&mut self)
    {
        unsafe{ gl::BindVertexArray(self.vao) }
    }

    /// Sets the format for the given vertex input layout, This function handles both integer and floating
    /// point vertex layout input. This function will NOT normalize the layout input
    ///
    /// `index` - the layout index that is being specified
    /// `count` - the number of elements per unit in the layout
    /// `data_type` - the type of data each layout element is
    /// `relative_offset` - the relative offset in the backing Mapped Buffer
    pub fn specify_layout_format(&mut self, index: u32, count: i32, data_type: GLenum, relative_offset: u32)
    {
        self.bind();
        unsafe
            {
                match data_type
                {
                    gl::FLOAT => gl::VertexAttribFormat(index, count, data_type, gl::FALSE, relative_offset),
                    gl::UNSIGNED_INT | gl::INT => gl::VertexAttribIFormat(index, count, data_type, relative_offset),
                    _ => unreachable!("Invalid data type parameter")
                }

                gl::VertexAttribBinding(index, index);
                gl::EnableVertexAttribArray(index);
            };
    }

    /// Sets the divisor for the given vertex layout input
    ///
    /// `index` - the layout index whose divisor is being specified
    /// `divisor` - the divisor of the layout
    pub fn specify_layout_divisor(&mut self, index: u32, divisor: u32)
    {
        self.bind();
        unsafe { gl::VertexAttribDivisor(index, divisor); }
    }
}