fn main()
{
    if cfg!(windows)
    {
        println!(r"cargo:rustc-link-search=.\ffi_dependencies\glfw");
    }
}