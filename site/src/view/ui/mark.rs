use damask::Component;

/// The wordmark's motif: a single damask ogee with its inner leaf.
///
/// Drawn rather than imported so it inherits `currentColor` and the size it is
/// given — the mark appears at 28px in the header and at 100px+ on the home
/// page, and a raster asset would need two files and a decision about which.
#[derive(Component)]
pub struct Mark {
    pub class: String,
}
