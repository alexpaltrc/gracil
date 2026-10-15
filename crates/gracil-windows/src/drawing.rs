//! How the window is drawn. Left to itself, the drawing library picks Vulkan on most graphics
//! cards. On Windows, Vulkan can stop handing back frames while the card is busy for hours with
//! compressing and the window sits behind others or minimized; the window then never draws again
//! while the work goes on underneath. Direct3D 12 is the graphics card's native way to draw on
//! Windows and doesn't stall like that, so Gracil asks for it.
//!
//! Left to itself, Iced also asks for the most powerful graphics card, which on a laptop with two
//! wakes the large one, warming the laptop and spending its battery for a window that draws a
//! few circles. Gracil asks for no card in particular, so Windows gives it the one it would give
//! any ordinary window (ADR 0066).

/// Asks the drawing library to use Direct3D 12, unless someone already chose otherwise through
/// `WGPU_BACKEND`, and no particular graphics card, unless `WGPU_POWER_PREF` chose one.
///
/// Call it first thing in `main`, before any other thread starts.
pub fn draw_with_direct3d() {
    for (name, value) in [("WGPU_BACKEND", "dx12"), ("WGPU_POWER_PREF", "none")] {
        if std::env::var_os(name).is_none() {
            // SAFETY: called at the very start of `main`, before any other thread exists, so
            // nothing can read or change the environment at the same time.
            #[allow(unsafe_code)]
            unsafe {
                std::env::set_var(name, value);
            }
        }
    }
}
