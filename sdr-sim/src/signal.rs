//! Signal generation and noise modeling

use ground_core::Frequency;

/// Type of signal to generate
#[derive(Debug, Clone, Copy)]
pub enum SignalType {
    /// Pure sine wave (carrier)
    SineWave,
    /// FSK modulated signal
    Fsk { baud_rate: u32 },
    /// QPSK modulated signal
    Qpsk { symbol_rate: u32 },
    /// Iridium burst
    IridiumBurst,
    /// NB-IoT NTN signal
    NbIotNtn,
}

/// Noise model for signal degradation
#[derive(Debug, Clone, Copy)]
pub enum NoiseModel {
    /// Additive white Gaussian noise
    Awgn { snr_db: f32 },
    /// Rayleigh fading
    RayleighFading { doppler_hz: f32 },
    /// Rician fading
    RicianFading { k_factor_db: f32, doppler_hz: f32 },
    /// No noise
    None,
}

/// Signal generator
pub struct SignalGenerator {
    signal_type: SignalType,
    noise_model: NoiseModel,
    carrier_frequency: Frequency,
    sample_rate: u64,
    phase: f64,
    time: f64,
}

impl SignalGenerator {
    pub fn new(
        signal_type: SignalType,
        noise_model: NoiseModel,
        carrier_frequency: Frequency,
        sample_rate: u64,
    ) -> Self {
        Self {
            signal_type,
            noise_model,
            carrier_frequency,
            sample_rate,
            phase: 0.0,
            time: 0.0,
        }
    }

    /// Generate the next sample (I, Q)
    pub fn next_sample(&mut self) -> (f32, f32) {
        let dt = 1.0 / self.sample_rate as f64;
        self.time += dt;

        // Generate base signal
        let (i, q) = match self.signal_type {
            SignalType::SineWave => self.generate_sine(),
            SignalType::Fsk { baud_rate } => self.generate_fsk(baud_rate),
            SignalType::Qpsk { symbol_rate } => self.generate_qpsk(symbol_rate),
            SignalType::IridiumBurst => self.generate_iridium(),
            SignalType::NbIotNtn => self.generate_nbiot(),
        };

        // Apply noise
        self.apply_noise(i, q)
    }

    /// Generate sine wave carrier
    fn generate_sine(&mut self) -> (f32, f32) {
        let angular_freq = 2.0 * std::f64::consts::PI * self.carrier_frequency as f64;
        self.phase =
            (self.phase + angular_freq / self.sample_rate as f64) % (2.0 * std::f64::consts::PI);

        let i = (self.phase).cos() as f32;
        let q = (self.phase).sin() as f32;

        (i, q)
    }

    /// Generate FSK signal
    fn generate_fsk(&mut self, baud_rate: u32) -> (f32, f32) {
        // Simplified FSK: alternate between two frequencies
        let symbol_duration = self.sample_rate as f64 / baud_rate as f64;
        let symbol_index = (self.time / symbol_duration) as u32;
        let freq_offset = if symbol_index.is_multiple_of(2) {
            1000.0
        } else {
            -1000.0
        };

        let angular_freq =
            2.0 * std::f64::consts::PI * (self.carrier_frequency as f64 + freq_offset);
        self.phase =
            (self.phase + angular_freq / self.sample_rate as f64) % (2.0 * std::f64::consts::PI);

        let i = (self.phase).cos() as f32;
        let q = (self.phase).sin() as f32;

        (i, q)
    }

    /// Generate QPSK signal
    fn generate_qpsk(&mut self, symbol_rate: u32) -> (f32, f32) {
        let symbol_duration = self.sample_rate as f64 / symbol_rate as f64;
        let symbol_index = (self.time / symbol_duration) as u32;

        // QPSK constellation points
        let constellation = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];
        let (i_base, q_base) = constellation[(symbol_index % 4) as usize];

        let angular_freq = 2.0 * std::f64::consts::PI * self.carrier_frequency as f64;
        self.phase =
            (self.phase + angular_freq / self.sample_rate as f64) % (2.0 * std::f64::consts::PI);

        let carrier_i = (self.phase).cos() as f32;
        let carrier_q = (self.phase).sin() as f32;

        // Mix constellation with carrier
        let i = i_base as f32 * carrier_i;
        let q = q_base as f32 * carrier_q;

        (i, q)
    }

    /// Generate Iridium-like burst signal
    fn generate_iridium(&mut self) -> (f32, f32) {
        // Iridium uses TDMA bursts
        let burst_duration = 0.090; // 90ms burst
        let cycle_time = 0.090 * 4.0; // 4 slots per frame

        let cycle_position = self.time % cycle_time;

        if cycle_position < burst_duration {
            // In burst: generate modulated signal
            self.generate_qpsk(24000) // 24 kbps
        } else {
            // Between bursts: silence
            (0.0, 0.0)
        }
    }

    /// Generate NB-IoT NTN-like signal
    fn generate_nbiot(&mut self) -> (f32, f32) {
        // NB-IoT uses SC-FDMA with continuous transmission
        self.generate_sine() // Simplified as continuous carrier
    }

    /// Apply noise model to signal
    fn apply_noise(&mut self, i: f32, q: f32) -> (f32, f32) {
        match self.noise_model {
            NoiseModel::Awgn { snr_db } => self.apply_awgn(i, q, snr_db),
            NoiseModel::RayleighFading { doppler_hz } => self.apply_rayleigh(i, q, doppler_hz),
            NoiseModel::RicianFading {
                k_factor_db,
                doppler_hz,
            } => self.apply_rician(i, q, k_factor_db, doppler_hz),
            NoiseModel::None => (i, q),
        }
    }

    /// Apply AWGN
    fn apply_awgn(&self, i: f32, q: f32, snr_db: f32) -> (f32, f32) {
        let signal_power = (i * i + q * q).sqrt();
        let snr_linear = 10.0_f32.powf(snr_db / 10.0);
        let noise_power = signal_power / snr_linear;
        let noise_std = noise_power.sqrt();

        let noise_i: f32 = rand::random::<f32>() * noise_std * 2.0 - noise_std;
        let noise_q: f32 = rand::random::<f32>() * noise_std * 2.0 - noise_std;

        (i + noise_i, q + noise_q)
    }

    /// Apply Rayleigh fading
    fn apply_rayleigh(&mut self, i: f32, q: f32, doppler_hz: f32) -> (f32, f32) {
        let dt = 1.0 / self.sample_rate as f64;
        let fading_rate = doppler_hz as f64 * dt;

        // Simplified Rayleigh fading
        let fade = (self.time * fading_rate * 2.0 * std::f64::consts::PI).sin() as f32 * 0.5 + 0.5;

        (i * fade, q * fade)
    }

    /// Apply Rician fading
    fn apply_rician(&mut self, i: f32, q: f32, k_factor_db: f32, doppler_hz: f32) -> (f32, f32) {
        let dt = 1.0 / self.sample_rate as f64;
        let fading_rate = doppler_hz as f64 * dt;
        let k_factor = 10.0_f32.powf(k_factor_db / 10.0);

        // Rician: strong LOS component + scattered component
        let los = 1.0;
        let scattered =
            (self.time * fading_rate * 2.0 * std::f64::consts::PI).sin() as f32 * 0.5 + 0.5;

        let fade = (los + k_factor * scattered) / (1.0 + k_factor);

        (i * fade, q * fade)
    }
}
