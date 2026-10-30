// Beat analysis of raw samples: the same maths as the Web Audio analyser the
// browser bridge uses, so the glow looks alike with every source.
use super::AudioFrame;
use std::time::Instant;

pub const N: usize = 1024;

pub struct Analyzer {
    rate: f32,
    window: Vec<f32>,
    smooth: Vec<f32>,
    re: Vec<f32>,
    im: Vec<f32>,
    bass_avg: f32,
    last_beat: Option<Instant>,
    gaps: Vec<f32>,
}

impl Analyzer {
    pub fn new(rate: u32) -> Self {
        Self {
            rate: rate as f32,
            window: (0..N).map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (N - 1) as f32).cos())).collect(),
            smooth: vec![0.0; N / 2],
            re: vec![0.0; N],
            im: vec![0.0; N],
            bass_avg: 0.0,
            last_beat: None,
            gaps: Vec::new(),
        }
    }

    /// `ring` holds the last N mono samples, oldest at `pos`. None = silence.
    pub fn frame(&mut self, ring: &[f32], pos: usize) -> Option<AudioFrame> {
        let mut peak = 0.0f32;
        for i in 0..N {
            let v = ring[(pos + i) % N];
            peak = peak.max(v.abs());
            self.re[i] = v * self.window[i];
            self.im[i] = 0.0;
        }
        if peak < 1e-4 {
            return None;
        }
        fft(&mut self.re, &mut self.im);
        for k in 0..N / 2 {
            let mag = (self.re[k] * self.re[k] + self.im[k] * self.im[k]).sqrt() / N as f32;
            self.smooth[k] = 0.6 * self.smooth[k] + 0.4 * mag;
        }
        let bin_hz = self.rate / N as f32;
        let bin = |hz: f32| ((hz / bin_hz).round() as usize).clamp(1, N / 2 - 1);
        let avg = |from: usize, to: usize| {
            let mut s = 0.0;
            for k in from..to.max(from + 1) {
                // Same scale as AnalyserNode.getByteFrequencyData (-100..-30 dB).
                let db = 20.0 * (self.smooth[k] + 1e-12).log10();
                s += ((db + 100.0) / 70.0).clamp(0.0, 1.0);
            }
            s / (to.max(from + 1) - from) as f32
        };
        let bass = avg(bin(40.0), bin(160.0));
        let energy = avg(bin(40.0), bin(8000.0));
        let edges = [40.0, 160.0, 500.0, 1500.0, 4000.0, 10000.0];
        let bands: Vec<f32> = (0..5).map(|i| avg(bin(edges[i]), bin(edges[i + 1]))).collect();

        let now = Instant::now();
        let since = self.last_beat.map(|t| now.duration_since(t).as_secs_f32() * 1000.0).unwrap_or(f32::MAX);
        let mut beat = false;
        // A rise over the running average, not a ratio: on loud masters the
        // level sits near the top of the scale and a ratio never triggers.
        if bass > self.bass_avg + 0.06 && bass > 0.3 && since > 280.0 {
            beat = true;
            if since < 1500.0 {
                self.gaps.push(since);
                if self.gaps.len() > 12 {
                    self.gaps.remove(0);
                }
            }
            self.last_beat = Some(now);
        }
        self.bass_avg = self.bass_avg * 0.92 + bass * 0.08;
        let bpm = if self.gaps.len() >= 4 {
            let mut g = self.gaps.clone();
            g.sort_by(f32::total_cmp);
            60000.0 / g[g.len() / 2]
        } else {
            0.0
        };
        Some(AudioFrame { energy, bass, beat, bands, bpm })
    }
}

fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f32::consts::PI / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        for i in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let (a, b) = (i + k, i + k + len / 2);
                let xr = re[b] * cr - im[b] * ci;
                let xi = re[b] * ci + im[b] * cr;
                re[b] = re[a] - xr;
                im[b] = im[a] - xi;
                re[a] += xr;
                im[a] += xi;
                let nr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = nr;
            }
        }
        len <<= 1;
    }
}
