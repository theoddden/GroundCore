// Pointing math and visibility
//
// Pointing and visibility computation is mathematically intensive and benefits
// from clean separation.

pub mod attenuation;
pub mod doppler;
pub mod pointing;
pub mod visibility;

pub use attenuation::{
    AtmosphericPath, AttenuationDb, WeatherConditions, compute_atmospheric_attenuation,
};
pub use doppler::{DopplerPrediction, compute_optical_doppler};
pub use pointing::{PointingVector, SatellitePosition, compute_pointing_vector};
pub use visibility::{VisibilityConstraints, VisibilityWindow, predict_visibility_window};
