//! How the window is drawn. Left to itself, the drawing library picks Vulkan on most graphics
//! cards. On Windows, Vulkan can stop handing back frames while the card is busy for hours with
//! compressing and the window sits behind others or minimized; the window then never draws again
//! while the work goes on underneath. Direct3D 12 is the graphics card's native way to draw on
//! Windows and doesn't stall like that, so Gracil asks for it.

/// Asks the drawing library to use Direct3D 12, unless someone already chose otherwise through
/// `WGPU_BACKEND`.
///
/// Call it first thing in `main`, before any other thread starts.
pub fn draw_with_direct3d() {
    if std::env::var_os("WGPU_BACKEND").is_none() {
        // SAFETY: called at the very start of `main`, before any other thread exists, so nothing
        // can read or change the environment at the same time.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("WGPU_BACKEND", "dx12");
        }
    }
}
