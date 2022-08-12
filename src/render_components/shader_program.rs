use std::ffi::CString;
use std::fmt::Debug;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::Path;
use gl::types::GLenum;

/// Representation of a shader program used in a render system
pub struct ShaderProgram
{
    pub shader_program: u32
}

/// Structure with required information to create a shader
pub struct ShaderInitInformation
{
    shader_type: GLenum,
    source: String,
}

impl ShaderProgram
{
    /// Creates a new shader program from the shaders that will be created from reading the function input.
    /// At the minimum, a vertex and fragment shader must be provided
    ///
    /// `shaders` - the information required to create shaders for the shader program
    pub fn new(shaders: &Vec<ShaderInitInformation>) -> Result<ShaderProgram, String>
    {
        let shaders =
            {
                let mut created_shaders = Vec::new();
                for x in shaders
                {
                    created_shaders.push(ShaderProgram::create_shader(x.shader_type, x.source.clone())?)
                }

                created_shaders
            };

        let shader_program = ShaderProgram::create_from_shaders(shaders)?;

        Ok( ShaderProgram{ shader_program } )
    }

    /// Uses the shader program; binds the shader program
    pub fn use_shader_program(&mut self)
    {
        unsafe{ gl::UseProgram(self.shader_program) }
    }

    /// Creates a shader program with the required information
    ///
    /// `shader_type` - the type of shader being created
    /// `shader_source` - the source of the shader that is to be compiled
    fn create_shader(shader_type: gl::types::GLenum, shader_source: String) -> Result<gl::types::GLenum, String>
    {
        let shader: gl::types::GLenum;

        let shader_shader_c_equivalent = match CString::new(shader_source)
        {
            Ok(i) => i,
            Err(_) => return Err("Unable to create a c-string from the passed in Rust String".to_string())
        };

        unsafe
            {
                shader = gl::CreateShader(shader_type);
                gl::ShaderSource(shader, 1, &shader_shader_c_equivalent.as_ptr(), std::ptr::null());
                gl::CompileShader(shader);
            }

        if let Some(error_message) = ShaderProgram::check_shader_compilation(shader)
        {
            return Err(error_message);
        }

        Ok(shader)
    }

    /// Checks if the given shader has been successfully compiled. Returns an error if the given
    /// shader could not be compiled
    ///
    /// `shader_type` - the type of shader having its compilation status checked
    fn check_shader_compilation(shader_type: gl::types::GLenum) -> Option<String>
    {
        let mut success: gl::types::GLint = 1;

        unsafe
            {
                gl::GetShaderiv(shader_type, gl::COMPILE_STATUS, &mut success);

                if success == 0
                {
                    let mut error_message_length: gl::types::GLint = 0;

                    gl::GetShaderiv(shader_type, gl::INFO_LOG_LENGTH, &mut error_message_length);

                    let mut error_message_buffer: Vec<u8> = Vec::with_capacity(error_message_length as usize + 1);

                    for _ in 0..error_message_buffer.capacity()
                    {
                        error_message_buffer.push(b' ');
                    }

                    let error_message = CString::from_vec_unchecked(error_message_buffer);

                    gl::GetShaderInfoLog(shader_type, error_message_length, std::ptr::null_mut(), error_message.as_ptr() as *mut gl::types::GLchar);

                    return Some(error_message.to_string_lossy().into_owned());
                }
            }

        None
    }

    /// Creates a shader program from the given shaders. Returns an error if the shader program
    /// could not link the provided shaders
    ///
    /// `shaders` - the successfully compiled shaders that will make up the shader program
    fn create_from_shaders(shaders: Vec<GLenum>) -> Result<GLenum, String>
    {
        let shader_program: GLenum;

        unsafe
            {
                shader_program = gl::CreateProgram();

                for x in shaders
                {
                    gl::AttachShader(shader_program, x);
                }

                gl::LinkProgram(shader_program);

                if let Some(error_message) = ShaderProgram::check_linkage(shader_program)
                {
                    return Err(error_message);
                }
            }

        Ok(shader_program)
    }

    /// Checks if the given shader program has been successfully linked
    ///
    /// `shader_program` - the shader program whose linkage status should be checked for
    fn check_linkage(shader_program: gl::types::GLenum) -> Option<String>
    {
        let mut success: gl::types::GLint = 1;

        unsafe {
            gl::GetProgramiv(shader_program, gl::LINK_STATUS, &mut success);

            if success == 0
            {
                let mut error_message_length: gl::types::GLint = 0;

                gl::GetProgramiv
                    (
                        shader_program,
                        gl::INFO_LOG_LENGTH,
                        &mut error_message_length,
                    );

                let mut error_message_buffer: Vec<u8> =
                    Vec::with_capacity(error_message_length as usize + 1);

                for _ in 0..error_message_buffer.capacity() {
                    error_message_buffer.push(b' ');
                }

                let error_message = CString::from_vec_unchecked(error_message_buffer);

                gl::GetProgramInfoLog(
                    shader_program,
                    error_message_length,
                    std::ptr::null_mut(),
                    error_message.as_ptr() as *mut gl::types::GLchar,
                );

                return Some(error_message.to_string_lossy().into_owned());
            }
        }

        None
    }
}

impl ShaderInitInformation
{
    /// Specifies the information to create a shader with the shader source being the given file
    ///
    /// `shader_type` - the type of shader to create
    /// `file_location` - the location of the file containing the shader source
    /// `append_contents` - shader source to append to the read shader source from the provided file
    /// `write_generated` - if relevant, location to write the generated shader source to
    pub fn from_file<A: AsRef<Path> + Debug + Clone, U: Into<String> + Debug + Clone>
    (shader_type: GLenum, file_location: A, append_contents: Option<U>, write_generated: Option<String>) -> Result<ShaderInitInformation, String>
    {
        let location = file_location.clone();
        let file = match File::open(file_location)
        {
            Ok(i) => i,
            Err(err) =>
                {
                    return Err(format!("Error opening file {:?}: {}", location, err.to_string()))
                }
        };

        let mut file_contents = String::new();

        let mut buf_reader = BufReader::new(file);

        if let Err(err) =  buf_reader.read_to_string(&mut file_contents)
        {
            return Err(err.to_string());
        }

        let total_shader_source = if let Some(append) = append_contents
        {
            append.into() + &file_contents
        }
        else
        {
            file_contents
        };

        if let Some(generated_name) = write_generated
        {
            let mut generated_file = File::create(generated_name).unwrap();
            generated_file.write_all(total_shader_source.as_bytes())
                .unwrap_or_else(|e| panic!("Failed to write generated shader: {}", e));
        }

        Ok( ShaderInitInformation { shader_type, source: total_shader_source } )
    }
}