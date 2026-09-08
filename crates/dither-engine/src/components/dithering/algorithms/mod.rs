mod error_diffusion;
mod halftone;
mod ordered;

pub(crate) use error_diffusion::render_error_diffusion;
pub(crate) use halftone::render_halftone;
pub(crate) use ordered::render_ordered;
