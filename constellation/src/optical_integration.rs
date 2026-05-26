//! Integration with optical tracking module

#[cfg(feature = "optical-integration")]
use crate::satellite::OrbitalState;
#[cfg(feature = "optical-integration")]
use optical::geometry::PointingVector;

#[cfg(feature = "optical-integration")]
impl OrbitalState {
    /// Convert to optical PointingVector
    pub fn to_pointing_vector(
        &self,
        station_lat_deg: f64,
        station_lon_deg: f64,
        station_alt_m: f64,
    ) -> PointingVector {
        // Convert station to ECI
        let station_eci =
            self.geodetic_to_eci(station_lat_deg, station_lon_deg, station_alt_m, self.time);

        // Compute range vector
        let range = [
            self.position[0] - station_eci[0],
            self.position[1] - station_eci[1],
            self.position[2] - station_eci[2],
        ];

        // Convert to ENU (East-North-Up)
        let lat = station_lat_deg.to_radians();
        let lon = station_lon_deg.to_radians();
        let sin_lat = lat.sin();
        let cos_lat = lat.cos();
        let sin_lon = lon.sin();
        let cos_lon = lon.cos();

        let east = -sin_lon * range[0] + cos_lon * range[1];
        let north =
            -sin_lat * cos_lon * range[0] - sin_lat * sin_lon * range[1] + cos_lat * range[2];
        let up = cos_lat * cos_lon * range[0] + cos_lat * sin_lon * range[1] + sin_lat * range[2];

        // Azimuth and elevation
        let azimuth = east.atan2(north).to_degrees();
        let elevation = (up / (east * east + north * north + up * up).sqrt())
            .asin()
            .to_degrees();
        let range_km = (range[0].powi(2) + range[1].powi(2) + range[2].powi(2)).sqrt();

        PointingVector {
            azimuth_rad: azimuth.to_radians(),
            elevation_rad: elevation.to_radians(),
            range_km,
        }
    }

    /// Convert geodetic to ECI (helper function)
    fn geodetic_to_eci(
        &self,
        lat_deg: f64,
        lon_deg: f64,
        alt_m: f64,
        time: chrono::DateTime<chrono::Utc>,
    ) -> [f64; 3] {
        let lat = lat_deg.to_radians();
        let lon = lon_deg.to_radians();
        let alt_km = alt_m / 1000.0;

        const RE: f64 = 6378.137;
        const FLATTENING: f64 = 1.0 / 298.257223563;

        let sin_lat = lat.sin();
        let cos_lat = lat.cos();
        let radius = RE / (1.0 - FLATTENING * sin_lat * sin_lat).sqrt() + alt_km;

        let jd = time.timestamp() as f64 / 86400.0 + 2440587.5;
        let gmst = 280.46061837 + 360.98564736629 * (jd - 2451545.0);
        let gmst_rad = (gmst % 360.0).to_radians();

        let theta = lon + gmst_rad;
        let x = radius * cos_lat * theta.cos();
        let y = radius * cos_lat * theta.sin();
        let z = radius * sin_lat;

        [x, y, z]
    }
}
