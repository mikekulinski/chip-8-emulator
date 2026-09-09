use std::num::NonZero;
use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate, Source};
use std::time::Duration;

struct SquareWave {
    freq: f32,
    sample_rate: u32,
    num_sample: u32,
}

impl SquareWave {
    fn new(freq: f32, sample_rate: u32) -> Self {
        SquareWave { freq, sample_rate, num_sample: 0 }
    }
}

impl Iterator for SquareWave {
    type Item = f32; // rodio::Sample is just an alias for f32
    fn next(&mut self) -> Option<f32> {
        self.num_sample = self.num_sample.wrapping_add(1);
        let period = self.sample_rate as f32 / self.freq;
        let phase = (self.num_sample as f32 % period) / period;
        Some(if phase < 0.5 { 0.25 } else { -0.25 }) // low amplitude, avoid ear-splitting
    }
}

impl Source for SquareWave {
    fn current_span_len(&self) -> Option<usize> { None }
    fn channels(&self) -> ChannelCount { NonZero::new(1).unwrap() }
    fn sample_rate(&self) -> SampleRate { NonZero::new(self.sample_rate).unwrap() }
    fn total_duration(&self) -> Option<Duration> { None }
}

pub(crate) struct Audio {
    _stream: MixerDeviceSink, // must stay alive for the duration of playback
    player: Player,
}

impl Audio {
    pub(crate) fn new() -> Self {
        let stream_handle = DeviceSinkBuilder::open_default_sink()
            .expect("open default audio output");
        let player = Player::connect_new(stream_handle.mixer());
        player.append(SquareWave::new(440.0, 44100));
        player.pause(); // start silent
        Audio { _stream: stream_handle, player }
    }

    pub fn set_playing(&self, playing: bool) {
        if playing {
            if self.player.is_paused() {
                self.player.play();
            }
        } else {
            self.player.pause();
        }
    }
}