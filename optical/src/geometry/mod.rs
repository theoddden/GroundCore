// Pointing math and visibility
//
// Pointing and visibility computation is mathematically intensive and benefits
// from clean separation.

pub mod pointing;
pub mod visibility;
pub mod doppler;
pub mod attenuation;

pub use pointing::{PointingVector, compute_pointing_vector, SatellitePosition};
pub use visibility::{VisibilityWindow, VisibilityConstraints, predict_visibility_window};
pub use doppler::{compute_optical_doppler, DopplerPrediction};
pub use attenuation::{compute_atmospheric_attenuation, AtmosphericPath, AttenuationDb, WeatherConditions};
