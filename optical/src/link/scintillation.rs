// Atmospheric scintillation model
//
// Atmospheric attenuation gets discussed publicly. Scintillation does not,
// and it's worse. Even in clear conditions, the atmosphere causes rapid
// amplitude and phase fluctuations — fading of 5–15 dB on timescales of
// 10–100 ms. A link budget that says the link should work tells you nothing
// about the 20% of time it fades below threshold.
//
// This module models scintillation as a statistical distribution rather than
// a deterministic penalty. The predicted SNR is expressed as a distribution
// with mean, variance, and availability percentiles.
//
// Physics: Rytov variance σ_R² = 1.23 · Cn² · k^(7/6) · L^(11/6)
//   - Cn²: refractive-index structure constant (site-specific, m^{-2/3})
//   - k = 2π/λ: optical wavenumber (rad/m)
//   - L: propagation path length through turbulence (m)
//
// Aperture averaging reduces scintillation variance for receivers larger
// than the Fried coherence length r₀. The averaging factor A(D) drops
// the variance for D >> r₀ — this is why large apertures are operationally
// preferred even when diffraction-limited resolution isn't needed.

use serde::{Deserialize, Serialize};

/// Site-specific atmospheric parameters.
///
/// Cn² values by site quality:
///   - Excellent (Mauna Kea, La Palma): ~5e-16 m^{-2/3}
///   - Good observatory site:           ~1e-15 m^{-2/3}
///   - Typical rooftop/urban:           ~1e-14 m^{-2/3}
///   - Urban poor:                      ~1e-13 m^{-2/3}
///
/// Academic models routinely underpredict Cn² — real operators calibrate
/// from years of site-specific measurement data. Use conservative values
/// for link availability planning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtmosphericSite {
    /// Refractive-index structure constant (m^{-2/3})
    pub cn2: f64,
    /// Effective turbulence path length (m). For a slant path at elevation ε,
    /// use L = H_turb / sin(ε) where H_turb ≈ 20 km.
    pub path_length_m: f64,
    /// Wavelength (m). 1550 nm for C-band optical, 850 nm for visible.
    pub wavelength_m: f64,
    /// Receiver aperture diameter (m). Larger apertures average out scintillation.
    pub aperture_diameter_m: f64,
    /// Effective wind speed across the wavefront (m/s). Drives fade timescale.
    pub wind_speed_m_per_s: f64,
}

impl AtmosphericSite {
    /// Typical rooftop/urban installation. 30° elevation slant path.
    pub fn typical_rooftop(wavelength_m: f64, aperture_diameter_m: f64) -> Self {
        Self {
            cn2: 1e-14,
            path_length_m: 20_000.0 / (30.0_f64.to_radians()).sin(),
            wavelength_m,
            aperture_diameter_m,
            wind_speed_m_per_s: 10.0,
        }
    }

    /// Excellent high-altitude dry-climate site (Mauna Kea, Cerro Tololo, La Palma class).
    pub fn excellent_site(wavelength_m: f64, aperture_diameter_m: f64) -> Self {
        Self {
            cn2: 5e-16,
            path_length_m: 20_000.0 / (30.0_f64.to_radians()).sin(),
            wavelength_m,
            aperture_diameter_m,
            wind_speed_m_per_s: 7.0,
        }
    }

    /// Construct for a specific elevation angle (degrees).
    pub fn at_elevation(
        cn2: f64,
        elevation_deg: f64,
        wavelength_m: f64,
        aperture_diameter_m: f64,
        wind_speed_m_per_s: f64,
    ) -> Self {
        let h_turb_m = 20_000.0;
        let path_length_m = h_turb_m / elevation_deg.to_radians().sin().max(0.01);
        Self {
            cn2,
            path_length_m,
            wavelength_m,
            aperture_diameter_m,
            wind_speed_m_per_s,
        }
    }
}

/// Scintillation severity classification based on Rytov variance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScintillationSeverity {
    /// σ_R² < 0.1 — log-normal model accurate, deep fades rare
    Weak,
    /// 0.1 ≤ σ_R² < 0.3 — log-normal model still usable
    Moderate,
    /// 0.3 ≤ σ_R² < 1.0 — deep fades frequent, aperture averaging important
    Strong,
    /// σ_R² ≥ 1.0 — saturation regime, irradiance PDF significantly non-Gaussian
    Saturated,
}

impl ScintillationSeverity {
    fn from_rytov(sigma_r_sq: f64) -> Self {
        if sigma_r_sq < 0.1 {
            Self::Weak
        } else if sigma_r_sq < 0.3 {
            Self::Moderate
        } else if sigma_r_sq < 1.0 {
            Self::Strong
        } else {
            Self::Saturated
        }
    }
}

/// SNR expressed as a probability distribution rather than a deterministic value.
///
/// The "predicted SNR" from a link budget is the mean of a distribution whose
/// variance is driven by scintillation. This struct expresses that reality and
/// enables availability-based link planning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnrDistribution {
    /// Nominal (free-space + static atmospheric) SNR before scintillation (dB)
    pub nominal_snr_db: f64,
    /// Scintillation-induced standard deviation (dB). This is the operationally
    /// relevant number — links with std_dev > 3 dB require diversity or margin.
    pub scintillation_std_dev_db: f64,
    /// Rytov variance (dimensionless)
    pub rytov_variance: f64,
    /// Severity classification
    pub severity: ScintillationSeverity,
    /// Aperture averaging factor (0..1; 1.0 = point receiver, 0 = full averaging).
    /// Values below 0.3 indicate significant averaging benefit from the aperture.
    pub aperture_averaging_factor: f64,
    /// Coherence time of the fading channel (seconds).
    /// Fades last approximately this long — relevant for FEC interleaver depth.
    pub fade_coherence_time_s: f64,
}

impl SnrDistribution {
    /// SNR at the p-th percentile (0..1), assuming log-normal fading.
    ///
    /// For weak-to-moderate turbulence the log-normal model gives a reasonable
    /// estimate. For saturated turbulence this underestimates deep-fade probability
    /// — treat results as optimistic in that regime.
    pub fn percentile_db(&self, p: f64) -> f64 {
        let p = p.clamp(1e-6, 1.0 - 1e-6);
        let z = probit(p);
        self.nominal_snr_db + z * self.scintillation_std_dev_db
    }

    /// Probability that instantaneous SNR exceeds `threshold_db`.
    /// Returns link availability (0..1) under scintillation at that threshold.
    pub fn availability_at_threshold(&self, threshold_db: f64) -> f64 {
        if self.scintillation_std_dev_db < 1e-9 {
            return if self.nominal_snr_db >= threshold_db {
                1.0
            } else {
                0.0
            };
        }
        let margin = (self.nominal_snr_db - threshold_db) / self.scintillation_std_dev_db;
        normal_cdf(margin)
    }

    /// 10th percentile SNR — exceeded 90% of the time. Use for availability planning.
    pub fn p10_db(&self) -> f64 {
        self.percentile_db(0.10)
    }

    /// Median SNR (50th percentile).
    pub fn p50_db(&self) -> f64 {
        self.percentile_db(0.50)
    }

    /// 90th percentile SNR — exceeded only 10% of the time.
    pub fn p90_db(&self) -> f64 {
        self.percentile_db(0.90)
    }

    /// Expected fade duration for fades below `threshold_db` (seconds).
    ///
    /// Approximation: d̄ ≈ τ_c · P_fade / P_avail
    /// where τ_c is the coherence time.
    pub fn expected_fade_duration_s(&self, threshold_db: f64) -> f64 {
        let p_avail = self.availability_at_threshold(threshold_db);
        if p_avail >= 1.0 - 1e-9 {
            return 0.0;
        }
        let p_fade = 1.0 - p_avail;
        self.fade_coherence_time_s * p_fade / p_avail.max(1e-9)
    }

    /// Whether aperture diversity (multiple receivers) would materially help.
    /// True when scintillation is strong enough that a single aperture is
    /// insufficient and the fade coherence length is small relative to the
    /// station spacing.
    pub fn aperture_diversity_beneficial(&self) -> bool {
        matches!(
            self.severity,
            ScintillationSeverity::Strong | ScintillationSeverity::Saturated
        )
    }
}

/// Scintillation model: compute SNR distributions from atmospheric parameters.
pub struct ScintillationModel;

impl ScintillationModel {
    /// Rytov variance: σ_R² = 1.23 · Cn² · k^(7/6) · L^(11/6)
    pub fn rytov_variance(site: &AtmosphericSite) -> f64 {
        let k = 2.0 * std::f64::consts::PI / site.wavelength_m;
        1.23 * site.cn2 * k.powf(7.0 / 6.0) * site.path_length_m.powf(11.0 / 6.0)
    }

    /// Fried coherence length r₀ (m).
    ///
    /// r₀ = (0.423 · k² · Cn² · L)^{-3/5}
    /// Typical observatory values: 5–20 cm. Larger = better seeing.
    pub fn fried_parameter(site: &AtmosphericSite) -> f64 {
        let k = 2.0 * std::f64::consts::PI / site.wavelength_m;
        (0.423 * k * k * site.cn2 * site.path_length_m).powf(-3.0 / 5.0)
    }

    /// Aperture averaging factor A(D) ∈ [0, 1].
    ///
    /// A(D) ≈ [1 + 1.062 · (D/r₀)^{5/3}]^{-6/5}
    /// A → 1 for D << r₀ (point receiver), A → 0 for D >> r₀ (full averaging).
    pub fn aperture_averaging_factor(site: &AtmosphericSite) -> f64 {
        let r0 = Self::fried_parameter(site);
        let ratio = site.aperture_diameter_m / r0;
        (1.0 + 1.062 * ratio.powf(5.0 / 3.0)).powf(-6.0 / 5.0)
    }

    /// Fade coherence time τ_c ≈ 0.314 · r₀ / v_wind (seconds).
    pub fn fade_coherence_time(site: &AtmosphericSite) -> f64 {
        let r0 = Self::fried_parameter(site);
        0.314 * r0 / site.wind_speed_m_per_s.max(0.1)
    }

    /// Log-normal scintillation standard deviation (dB).
    ///
    /// σ_I_eff = A(D) · min(σ_R², 1) [clamp avoids overestimate in saturation regime]
    /// σ_dB = (10/ln10) · sqrt(ln(1 + σ_I_eff))
    pub fn scintillation_std_dev_db(site: &AtmosphericSite) -> f64 {
        let sigma_r_sq = Self::rytov_variance(site);
        let a = Self::aperture_averaging_factor(site);
        let sigma_i_eff = a * sigma_r_sq.min(1.0);
        let log_variance = (1.0 + sigma_i_eff).ln();
        (10.0 / std::f64::consts::LN_10) * log_variance.sqrt()
    }

    /// Compute a full SNR distribution for the given site and nominal link SNR.
    pub fn snr_distribution(site: &AtmosphericSite, nominal_snr_db: f64) -> SnrDistribution {
        let rytov = Self::rytov_variance(site);
        let std_dev = Self::scintillation_std_dev_db(site);
        let averaging = Self::aperture_averaging_factor(site);
        let coherence = Self::fade_coherence_time(site);
        let severity = ScintillationSeverity::from_rytov(rytov);

        SnrDistribution {
            nominal_snr_db,
            scintillation_std_dev_db: std_dev,
            rytov_variance: rytov,
            severity,
            aperture_averaging_factor: averaging,
            fade_coherence_time_s: coherence,
        }
    }
}

/// Rational approximation to the normal CDF Φ(x). Max error ≈ 7.5×10⁻⁸.
fn normal_cdf(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.2316419 * x.abs());
    let poly = t
        * (0.319381530
            + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));
    let pdf = (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let cdf = 1.0 - pdf * poly;
    if x >= 0.0 { cdf } else { 1.0 - cdf }
}

/// Rational approximation to the probit (inverse normal CDF). Abramowitz & Stegun 26.2.23.
fn probit(p: f64) -> f64 {
    let sign = if p < 0.5 { -1.0 } else { 1.0 };
    let q = p.min(1.0 - p);
    let t = (-2.0 * q.ln()).sqrt();
    let c = [2.515517_f64, 0.802853, 0.010328];
    let d = [1.432788_f64, 0.189269, 0.001308];
    let numerator = c[0] + c[1] * t + c[2] * t * t;
    let denominator = 1.0 + d[0] * t + d[1] * t * t + d[2] * t * t * t;
    sign * (t - numerator / denominator)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_site_quality_ordering_in_physics() {
        // For optical slant paths, σ_R² >> 1 (saturated regime) for all sites.
        // The operationally meaningful site-quality comparison is in the underlying
        // physics parameters, not the saturated std_dev (which is dominated by
        // aperture averaging rather than Cn²).
        //
        // A larger Fried parameter means more coherent wavefront = better seeing.
        // A lower Rytov variance means less turbulent energy = less scintillation.
        let excellent = AtmosphericSite::excellent_site(1550e-9, 0.1);
        let rooftop = AtmosphericSite::typical_rooftop(1550e-9, 0.1);

        let r0_excellent = ScintillationModel::fried_parameter(&excellent);
        let r0_rooftop = ScintillationModel::fried_parameter(&rooftop);
        assert!(
            r0_excellent > r0_rooftop,
            "Excellent site must have larger Fried parameter (better seeing): {r0_excellent:.4} m vs {r0_rooftop:.4} m"
        );

        let rytov_excellent = ScintillationModel::rytov_variance(&excellent);
        let rytov_rooftop = ScintillationModel::rytov_variance(&rooftop);
        assert!(
            rytov_excellent < rytov_rooftop,
            "Excellent site must have lower Rytov variance: {rytov_excellent:.1} vs {rytov_rooftop:.1}"
        );
    }

    #[test]
    fn test_availability_degrades_at_poor_site() {
        let site = AtmosphericSite::typical_rooftop(1550e-9, 0.05);
        let dist = ScintillationModel::snr_distribution(&site, 15.0);
        let avail = dist.availability_at_threshold(10.0);
        assert!(avail < 1.0, "Poor site should have <100% availability");
        assert!(
            avail > 0.5,
            "Should still be mostly available with 5 dB margin"
        );
    }

    #[test]
    fn test_aperture_averaging_reduces_variance() {
        let small = AtmosphericSite::typical_rooftop(1550e-9, 0.05);
        let large = AtmosphericSite::typical_rooftop(1550e-9, 0.5);
        let dist_small = ScintillationModel::snr_distribution(&small, 15.0);
        let dist_large = ScintillationModel::snr_distribution(&large, 15.0);
        assert!(
            dist_large.scintillation_std_dev_db < dist_small.scintillation_std_dev_db,
            "Larger aperture must reduce scintillation std dev: large={}, small={}",
            dist_large.scintillation_std_dev_db,
            dist_small.scintillation_std_dev_db
        );
    }

    #[test]
    fn test_percentile_ordering() {
        let site = AtmosphericSite::typical_rooftop(1550e-9, 0.1);
        let dist = ScintillationModel::snr_distribution(&site, 15.0);
        assert!(dist.p10_db() < dist.p50_db(), "p10 must be below median");
        assert!(dist.p50_db() < dist.p90_db(), "p90 must be above median");
    }

    #[test]
    fn test_fade_duration_increases_near_threshold() {
        let site = AtmosphericSite::typical_rooftop(1550e-9, 0.05);
        let dist = ScintillationModel::snr_distribution(&site, 15.0);
        let fade_tight = dist.expected_fade_duration_s(14.0);
        let fade_loose = dist.expected_fade_duration_s(5.0);
        assert!(
            fade_tight > fade_loose,
            "Tight threshold should produce longer fades"
        );
    }

    #[test]
    fn test_no_fade_when_large_snr_margin() {
        let site = AtmosphericSite::excellent_site(1550e-9, 0.2);
        let dist = ScintillationModel::snr_distribution(&site, 30.0);
        let avail = dist.availability_at_threshold(10.0);
        assert!(
            avail > 0.99,
            "20 dB margin on excellent site should give >99% availability"
        );
    }
}
