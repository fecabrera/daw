use crate::{BandMeter, MeterStorage, PeakMeter};

/// The current and held values shared by the plugin meter messages.
pub struct PluginMeterPrefix<'a> {
    pub output: &'a BandMeter<3>,
    pub reduction: &'a PeakMeter,
    pub bands: &'a BandMeter<3>,
}

impl PluginMeterPrefix<'_> {
    pub const VALUES: usize = 13;

    pub fn encode(&self) -> [f64; 13] {
        let Self {
            output,
            reduction,
            bands,
        } = self;
        let mut v = [0.0; Self::VALUES];
        v[0] = f64::from(output.readings()[0]);
        v[1] = f64::from(output.boost_peaks()[0].peak());
        v[2] = f64::from(reduction.reading());
        v[3] = f64::from(reduction.peak());
        for i in 0..3 {
            v[4 + i] = f64::from(bands.readings()[i]);
            v[7 + i] = f64::from(bands.reduction_peaks()[i].peak());
            v[10 + i] = f64::from(bands.boost_peaks()[i].peak());
        }
        v
    }

    pub fn receive(&self, v: &[f32; 13]) {
        let Self {
            output,
            reduction,
            bands,
        } = self;
        output.clear();
        output.store([v[1], 0.0, 0.0]);
        output.initialize([v[0], 0.0, 0.0]);
        reduction.clear();
        reduction.store(v[3]);
        reduction.initialize(v[2]);
        bands.clear();
        bands.store(std::array::from_fn(|i| -v[7 + i]));
        bands.store(std::array::from_fn(|i| v[10 + i]));
        bands.initialize(std::array::from_fn(|i| v[4 + i]));
    }

    pub fn reset(&self, mask: u32) {
        let Self {
            output,
            reduction,
            bands,
        } = self;
        if mask & 1 != 0 {
            output.boost_peaks()[0].reset_peak();
        }
        if mask & 2 != 0 {
            reduction.reset_peak();
        }
        for i in 0..3 {
            if mask & (4 << i) != 0 {
                bands.reduction_peaks()[i].reset_peak();
                bands.boost_peaks()[i].reset_peak();
            }
        }
    }
}
