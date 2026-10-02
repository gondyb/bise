//! The speaker's echo cancelled in the mic (owner: voice-echo; plan §8
//! #6, §8.1): on speakers the agent heard itself and answered itself.
//!
//! macOS: one VoiceProcessingIO AudioUnit (the unit FaceTime uses) is the
//! mic and the speaker together; it knows what it plays, so it removes
//! that from what the mic hears. Both sides run at [`RATE`] mono f32
//! (the TTS's rate: the voice is not resampled); the mic's side is
//! resampled to [`MIC_RATE`] and cut into the same 20 ms blocks as
//! `CpalMic` ([`MicPath`]); the speaker's side is a [`Playback`] behind
//! a `QueueSpeaker` (the same queue, clock, stop and level as the plain
//! speaker). [`open`] fails on any error (no unit, a property refused,
//! no device): `audio::open_voice_io` then opens the plain devices.
//!
//! The CoreAudio calls are declared here (the frameworks are linked by
//! cpal already; no new crate, as route.rs). What a test can check
//! without a device is pure: the stream format, the mic's path, the
//! speaker's fill, the status messages. Nothing here is opened by a test.

// a child of audio.rs (`audio::aec`): `super` is the audio module
use super::{Blocker, Playback, SharedLevel};
use crate::voicemode::{Mic, MicBlock, Speaker, MIC_RATE, TTS_RATE};
use crate::voice::{loudness, peak, Resampler};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The unit's rate on both sides (its client format): the TTS's.
pub const RATE: u32 = TTS_RATE;

/// A CoreAudio four-char code.
const fn fourcc(s: &[u8; 4]) -> u32 {
    ((s[0] as u32) << 24) | ((s[1] as u32) << 16) | ((s[2] as u32) << 8) | s[3] as u32
}

/// kAudioFormatLinearPCM
const FORMAT_LPCM: u32 = fourcc(b"lpcm");
/// kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked
const FLAGS_F32_PACKED: u32 = 0x1 | 0x8;

/// AudioStreamBasicDescription: what the unit gives us and takes from us.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StreamFormat {
    pub sample_rate: f64,
    pub format_id: u32,
    pub format_flags: u32,
    pub bytes_per_packet: u32,
    pub frames_per_packet: u32,
    pub bytes_per_frame: u32,
    pub channels_per_frame: u32,
    pub bits_per_channel: u32,
    pub reserved: u32,
}

impl StreamFormat {
    /// Mono f32 at `rate`.
    pub fn mono_f32(rate: u32) -> StreamFormat {
        StreamFormat {
            sample_rate: rate as f64,
            format_id: FORMAT_LPCM,
            format_flags: FLAGS_F32_PACKED,
            bytes_per_packet: 4,
            frames_per_packet: 1,
            bytes_per_frame: 4,
            channels_per_frame: 1,
            bits_per_channel: 32,
            reserved: 0,
        }
    }
}

/// AUVoiceIOOtherAudioDuckingConfiguration (macOS 14): the unit lowers
/// the other apps' sound while it runs; we ask for the least of it.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ducking {
    pub enable_advanced: u8,
    pub level: u32,
}

/// kAUVoiceIOOtherAudioDuckingLevelMin
pub const DUCKING_MIN: u32 = 10;

/// An OSStatus for a message: its four letters when it is a code
/// ('!pri'), else the number.
pub fn os_status(status: i32) -> String {
    let b = status.to_be_bytes();
    if b.iter().all(|c| c.is_ascii_graphic() || *c == b' ') {
        format!("'{}'", String::from_utf8_lossy(&b))
    } else {
        status.to_string()
    }
}

fn check(status: i32, what: &str) -> Result<(), String> {
    if status == 0 {
        Ok(())
    } else {
        Err(format!("echo cancelling: {} failed ({})", what, os_status(status)))
    }
}

/// The mic's side: the unit's [`RATE`] f32 buffers (any size) to
/// [`MIC_RATE`] i16 blocks of 20 ms, each with its capture time (as
/// `CpalMic`'s).
#[derive(Debug)]
pub struct MicPath {
    resampler: Resampler,
    blocker: Blocker,
}

impl Default for MicPath {
    fn default() -> Self {
        MicPath { resampler: Resampler::new(RATE, MIC_RATE), blocker: Blocker::default() }
    }
}

impl MicPath {
    /// `pcm` captured from `at` (its first sample).
    pub fn push(&mut self, pcm: &[f32], at: Instant) -> Vec<MicBlock> {
        let pcm = self.resampler.process(pcm);
        self.blocker.push(&pcm, at)
    }
}

/// Where the mic's blocks go once `Mic::open` was called; None: the
/// unit runs (the speaker plays) and what the mic hears is dropped.
pub struct MicSink {
    pub path: MicPath,
    pub blocks: std::sync::mpsc::Sender<MicBlock>,
}

/// The input callback's work after the unit rendered the mic: `pcm`
/// ends now. False: nobody listens any more (the sink is closed).
pub fn deliver(sink: &mut MicSink, level: &SharedLevel, pcm: &[f32], now: Instant) -> bool {
    let len = Duration::from_secs_f64(pcm.len() as f64 / RATE as f64);
    let at = now.checked_sub(len).unwrap_or(now);
    for b in sink.path.push(pcm, at) {
        level.set(loudness(peak(&b.pcm)));
        if sink.blocks.send(b).is_err() {
            return false;
        }
    }
    true
}

/// The render callback's work: the speaker's next samples, silence when
/// the queue is unusable (a poisoned lock never makes a noise).
pub fn fill(play: &Mutex<Playback>, out: &mut [f32]) {
    match play.lock() {
        Ok(mut p) => p.fill(out),
        Err(_) => out.fill(0.0),
    }
}

/// The speaker callback's buffer: exactly `frames` samples of the queue
/// (the frame count is what the unit plays; its buffer may be bigger),
/// the rest silent. Taking the whole buffer would drop the samples past
/// `frames` from the queue unheard (tts-lastword: the end of a sentence
/// skipped, the next one started at once).
pub fn speaker_out(play: &Mutex<Playback>, out: &mut [f32], frames: usize) {
    let n = frames.min(out.len());
    let (played, rest) = out.split_at_mut(n);
    fill(play, played);
    rest.fill(0.0);
}

/// `BISE_VOICE_AEC=0` (or off/false/no): the plain devices, no echo
/// cancelling (if the voice unit misbehaves on some Mac).
pub fn wanted(env: Option<&str>) -> bool {
    !matches!(env.map(|v| v.trim().to_ascii_lowercase()).as_deref(), Some("0" | "off" | "false" | "no"))
}

/// The mic and the speaker of one unit.
pub type Pair = (Box<dyn Mic>, Box<dyn Speaker>);

/// The mic and the speaker in one voice-processing unit, started.
#[cfg(target_os = "macos")]
pub fn open() -> Result<Pair, String> {
    mac::open()
}

#[cfg(not(target_os = "macos"))]
pub fn open() -> Result<Pair, String> {
    Err("echo cancelling needs macOS".into())
}

#[cfg(target_os = "macos")]
mod mac {
    //! The AudioUnit calls. One unit: element 1 is the mic (its output
    //! scope gives us the cancelled mic), element 0 the speaker (its
    //! input scope takes our voice).

    use super::{check, deliver, fourcc, speaker_out, Ducking, MicPath, MicSink, StreamFormat, DUCKING_MIN, RATE};
    use crate::voicemode::audio::{Playback, QueueSpeaker, SharedLevel};
    use crate::voicemode::{Mic, MicBlock, MicStream};
    use std::ffi::c_void;
    use std::sync::mpsc::Sender;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    type AudioUnit = *mut c_void;
    type RenderProc = extern "C" fn(*mut c_void, *mut u32, *const c_void, u32, u32, *mut AudioBufferList) -> i32;

    #[repr(C)]
    struct ComponentDescription {
        kind: u32,
        sub_kind: u32,
        manufacturer: u32,
        flags: u32,
        flags_mask: u32,
    }

    #[repr(C)]
    struct RenderCallback {
        proc_: RenderProc,
        ref_con: *mut c_void,
    }

    #[repr(C)]
    struct AudioBuffer {
        channels: u32,
        byte_size: u32,
        data: *mut c_void,
    }

    #[repr(C)]
    struct AudioBufferList {
        count: u32,
        buffers: [AudioBuffer; 1],
    }

    const TYPE_OUTPUT: u32 = fourcc(b"auou");
    const SUBTYPE_VOICE_PROCESSING_IO: u32 = fourcc(b"vpio");
    const MANUFACTURER_APPLE: u32 = fourcc(b"appl");
    const PROPERTY_STREAM_FORMAT: u32 = 8;
    const PROPERTY_MAX_FRAMES: u32 = 14;
    const PROPERTY_SET_RENDER_CALLBACK: u32 = 23;
    const PROPERTY_ENABLE_IO: u32 = 2003;
    const PROPERTY_SET_INPUT_CALLBACK: u32 = 2005;
    const PROPERTY_DUCKING: u32 = 2108;
    const SCOPE_GLOBAL: u32 = 0;
    const SCOPE_INPUT: u32 = 1;
    const SCOPE_OUTPUT: u32 = 2;
    const SPEAKER: u32 = 0;
    const MIC: u32 = 1;

    #[link(name = "AudioToolbox", kind = "framework")]
    extern "C" {
        fn AudioComponentFindNext(after: *mut c_void, desc: *const ComponentDescription) -> *mut c_void;
        fn AudioComponentInstanceNew(component: *mut c_void, out: *mut AudioUnit) -> i32;
        fn AudioComponentInstanceDispose(unit: AudioUnit) -> i32;
        fn AudioUnitSetProperty(unit: AudioUnit, id: u32, scope: u32, element: u32, data: *const c_void, size: u32) -> i32;
        fn AudioUnitGetProperty(unit: AudioUnit, id: u32, scope: u32, element: u32, data: *mut c_void, size: *mut u32) -> i32;
        fn AudioUnitInitialize(unit: AudioUnit) -> i32;
        fn AudioUnitUninitialize(unit: AudioUnit) -> i32;
        fn AudioOutputUnitStart(unit: AudioUnit) -> i32;
        fn AudioOutputUnitStop(unit: AudioUnit) -> i32;
        fn AudioUnitRender(unit: AudioUnit, flags: *mut u32, time: *const c_void, bus: u32, frames: u32, data: *mut AudioBufferList) -> i32;
    }

    fn description() -> ComponentDescription {
        ComponentDescription {
            kind: TYPE_OUTPUT,
            sub_kind: SUBTYPE_VOICE_PROCESSING_IO,
            manufacturer: MANUFACTURER_APPLE,
            flags: 0,
            flags_mask: 0,
        }
    }

    /// The voice-processing unit is on this Mac (looks it up; opens
    /// nothing).
    pub fn available() -> bool {
        // SAFETY: the description lives for the call; a null `after`
        // starts the search
        !unsafe { AudioComponentFindNext(std::ptr::null_mut(), &description()) }.is_null()
    }

    /// What the mic's callback works with.
    struct InputCtx {
        unit: AudioUnit,
        buf: Vec<f32>,
        sink: Arc<Mutex<Option<MicSink>>>,
        level: Arc<SharedLevel>,
    }

    /// The unit and its callbacks' contexts; dropped: stopped, disposed,
    /// then the contexts freed (no callback runs after the dispose).
    struct Unit {
        unit: AudioUnit,
        initialized: bool,
        started: bool,
        input: *mut InputCtx,
        render: *mut Arc<Mutex<Playback>>,
    }

    // SAFETY: the AudioUnit API may be called from any thread; the
    // contexts are only touched by the unit's callbacks until the drop,
    // which stops and disposes the unit before freeing them.
    unsafe impl Send for Unit {}
    unsafe impl Sync for Unit {}

    impl Drop for Unit {
        fn drop(&mut self) {
            // SAFETY: `unit` came from AudioComponentInstanceNew (or is
            // null); each step undoes one that succeeded
            unsafe {
                if !self.unit.is_null() {
                    if self.started {
                        AudioOutputUnitStop(self.unit);
                    }
                    if self.initialized {
                        AudioUnitUninitialize(self.unit);
                    }
                    AudioComponentInstanceDispose(self.unit);
                }
                if !self.input.is_null() {
                    drop(Box::from_raw(self.input));
                }
                if !self.render.is_null() {
                    drop(Box::from_raw(self.render));
                }
            }
        }
    }

    impl Unit {
        fn set<T>(&self, id: u32, scope: u32, element: u32, value: &T, what: &str) -> Result<(), String> {
            // SAFETY: `value` is a T of size_of::<T>() bytes for the call
            let status = unsafe {
                AudioUnitSetProperty(self.unit, id, scope, element, value as *const T as *const c_void, std::mem::size_of::<T>() as u32)
            };
            check(status, what)
        }

        fn max_frames(&self) -> u32 {
            let mut n: u32 = 0;
            let mut size = std::mem::size_of::<u32>() as u32;
            // SAFETY: `n` is a u32 of `size` bytes
            let status = unsafe {
                AudioUnitGetProperty(self.unit, PROPERTY_MAX_FRAMES, SCOPE_GLOBAL, 0, &mut n as *mut u32 as *mut c_void, &mut size)
            };
            if status == 0 { n } else { 0 }
        }
    }

    extern "C" fn on_mic(ref_con: *mut c_void, flags: *mut u32, time: *const c_void, bus: u32, frames: u32, _data: *mut AudioBufferList) -> i32 {
        // SAFETY: `ref_con` is the InputCtx given with the callback; it
        // lives until the unit is disposed
        let ctx = unsafe { &mut *(ref_con as *mut InputCtx) };
        let n = frames as usize;
        if ctx.buf.len() < n {
            ctx.buf.resize(n, 0.0);
        }
        let mut list = AudioBufferList {
            count: 1,
            buffers: [AudioBuffer { channels: 1, byte_size: (n * 4) as u32, data: ctx.buf.as_mut_ptr() as *mut c_void }],
        };
        // SAFETY: the list points at `n` f32s of ctx.buf; the unit's
        // flags and time are passed through
        let status = unsafe { AudioUnitRender(ctx.unit, flags, time, bus, frames, &mut list) };
        if status != 0 {
            return status;
        }
        let now = Instant::now();
        let got = (list.buffers[0].byte_size as usize / 4).min(n);
        if let Ok(mut slot) = ctx.sink.lock() {
            if let Some(sink) = slot.as_mut() {
                if !deliver(sink, &ctx.level, &ctx.buf[..got], now) {
                    *slot = None;
                }
            }
        }
        0
    }

    extern "C" fn on_speaker(ref_con: *mut c_void, _flags: *mut u32, _time: *const c_void, _bus: u32, frames: u32, data: *mut AudioBufferList) -> i32 {
        // SAFETY: `ref_con` is the Playback given with the callback (it
        // lives until the dispose); `data` is the unit's list of
        // `count` buffers (declared with one, read up to `count`)
        unsafe {
            let play = &*(ref_con as *const Arc<Mutex<Playback>>);
            if data.is_null() {
                return 0;
            }
            let count = (*data).count as usize;
            let first = (*data).buffers.as_mut_ptr();
            for i in 0..count {
                let b = &mut *first.add(i);
                if b.data.is_null() {
                    continue;
                }
                let out = std::slice::from_raw_parts_mut(b.data as *mut f32, b.byte_size as usize / 4);
                if i == 0 {
                    speaker_out(play, out, frames as usize);
                } else {
                    out.fill(0.0);
                }
            }
        }
        0
    }

    /// Holds the unit for the speaker (QueueSpeaker's device).
    struct Keep(#[allow(dead_code)] Arc<Unit>);

    pub struct VoiceMic {
        unit: Arc<Unit>,
        sink: Arc<Mutex<Option<MicSink>>>,
        level: Arc<SharedLevel>,
    }

    struct VoiceMicStream {
        sink: Arc<Mutex<Option<MicSink>>>,
        level: Arc<SharedLevel>,
        _unit: Arc<Unit>,
    }

    impl Drop for VoiceMicStream {
        fn drop(&mut self) {
            if let Ok(mut s) = self.sink.lock() {
                *s = None;
            }
        }
    }

    impl MicStream for VoiceMicStream {
        fn level(&self) -> f32 {
            self.level.get()
        }
    }

    impl Mic for VoiceMic {
        fn open(&mut self, blocks: Sender<MicBlock>) -> Result<Box<dyn MicStream>, String> {
            let mut s = self.sink.lock().map_err(|_| "the mic is unavailable".to_string())?;
            *s = Some(MicSink { path: MicPath::default(), blocks });
            Ok(Box::new(VoiceMicStream { sink: self.sink.clone(), level: self.level.clone(), _unit: self.unit.clone() }))
        }
    }

    pub fn open() -> Result<super::Pair, String> {
        // SAFETY: the description lives for the call
        let component = unsafe { AudioComponentFindNext(std::ptr::null_mut(), &description()) };
        if component.is_null() {
            return Err("echo cancelling: no voice-processing unit".into());
        }
        let mut unit = Unit { unit: std::ptr::null_mut(), initialized: false, started: false, input: std::ptr::null_mut(), render: std::ptr::null_mut() };
        // SAFETY: `component` was found above; `unit.unit` receives the instance
        check(unsafe { AudioComponentInstanceNew(component, &mut unit.unit) }, "creating the unit")?;
        if unit.unit.is_null() {
            return Err("echo cancelling: no unit".into());
        }
        let on: u32 = 1;
        unit.set(PROPERTY_ENABLE_IO, SCOPE_INPUT, MIC, &on, "enabling the mic")?;
        unit.set(PROPERTY_ENABLE_IO, SCOPE_OUTPUT, SPEAKER, &on, "enabling the speaker")?;
        let format = StreamFormat::mono_f32(RATE);
        unit.set(PROPERTY_STREAM_FORMAT, SCOPE_OUTPUT, MIC, &format, "the mic's format")?;
        unit.set(PROPERTY_STREAM_FORMAT, SCOPE_INPUT, SPEAKER, &format, "the speaker's format")?;

        let play = Arc::new(Mutex::new(Playback::new(RATE)));
        let sink: Arc<Mutex<Option<MicSink>>> = Arc::new(Mutex::new(None));
        let level = Arc::new(SharedLevel::default());
        let frames = (unit.max_frames() as usize).max(4096);
        unit.input = Box::into_raw(Box::new(InputCtx { unit: unit.unit, buf: vec![0.0; frames], sink: sink.clone(), level: level.clone() }));
        unit.render = Box::into_raw(Box::new(play.clone()));
        let input = RenderCallback { proc_: on_mic, ref_con: unit.input as *mut c_void };
        unit.set(PROPERTY_SET_INPUT_CALLBACK, SCOPE_GLOBAL, SPEAKER, &input, "the mic's callback")?;
        let render = RenderCallback { proc_: on_speaker, ref_con: unit.render as *mut c_void };
        unit.set(PROPERTY_SET_RENDER_CALLBACK, SCOPE_INPUT, SPEAKER, &render, "the speaker's callback")?;
        // the other apps' sound lowered as little as the unit allows
        // (macOS 14; older ones refuse it: their default)
        let _ = unit.set(PROPERTY_DUCKING, SCOPE_GLOBAL, 0, &Ducking { enable_advanced: 0, level: DUCKING_MIN }, "ducking");

        // SAFETY: the unit is set up above; each success is recorded for the drop
        check(unsafe { AudioUnitInitialize(unit.unit) }, "initializing the unit")?;
        unit.initialized = true;
        check(unsafe { AudioOutputUnitStart(unit.unit) }, "starting the unit")?;
        unit.started = true;

        let unit = Arc::new(unit);
        let mic = VoiceMic { unit: unit.clone(), sink, level };
        let speaker = QueueSpeaker::new(play, Some(Box::new(Keep(unit))));
        Ok((Box::new(mic), Box::new(speaker)))
    }
}

/// The voice-processing unit exists on this Mac (a lookup: nothing is
/// opened, nothing plays).
pub fn available() -> bool {
    #[cfg(target_os = "macos")]
    {
        mac::available()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn the_stream_format_is_coreaudios_mono_f32() {
        let f = StreamFormat::mono_f32(RATE);
        assert_eq!(std::mem::size_of::<StreamFormat>(), 40);
        assert_eq!(f.sample_rate, 24_000.0);
        assert_eq!(f.format_id, 0x6c70_636d); // 'lpcm'
        assert_eq!(f.format_flags, 9); // float | packed
        assert_eq!((f.bytes_per_frame, f.bytes_per_packet, f.frames_per_packet), (4, 4, 1));
        assert_eq!((f.channels_per_frame, f.bits_per_channel), (1, 32));
        // AUVoiceIOOtherAudioDuckingConfiguration: a Boolean then a UInt32
        assert_eq!(std::mem::size_of::<Ducking>(), 8);
    }

    #[test]
    fn the_voice_is_not_resampled() {
        assert_eq!(RATE, TTS_RATE);
    }

    #[test]
    fn the_mic_path_gives_20_ms_blocks_at_16_khz_with_their_time() {
        let mut path = MicPath::default();
        let t0 = Instant::now();
        // 10 ms device buffers at 24 kHz: 240 samples each
        let mut blocks = Vec::new();
        for i in 0..10u32 {
            blocks.extend(path.push(&[0.25; 240], t0 + Duration::from_millis(10) * i));
        }
        // 100 ms in: 1600 samples at 16 kHz, five blocks of 320
        assert_eq!(blocks.len(), 5);
        assert!(blocks.iter().all(|b| b.pcm.len() == 320));
        assert!(blocks.iter().all(|b| b.pcm.iter().all(|&s| (s - 8192).abs() <= 1)));
        let at: Vec<u128> = blocks.iter().map(|b| (b.at - t0).as_millis()).collect();
        assert_eq!(at, vec![0, 20, 40, 60, 80]);
    }

    #[test]
    fn deliver_sends_the_blocks_sets_the_level_and_says_when_nobody_listens() {
        let (tx, rx) = mpsc::channel();
        let mut sink = MicSink { path: MicPath::default(), blocks: tx };
        let level = SharedLevel::default();
        let now = Instant::now() + Duration::from_secs(1);
        // 40 ms ending now
        assert!(deliver(&mut sink, &level, &[0.5; 960], now));
        let got: Vec<MicBlock> = rx.try_iter().collect();
        assert_eq!(got.len(), 2);
        assert_eq!(now - got[0].at, Duration::from_millis(40));
        assert!(level.get() > 0.0);
        drop(rx);
        assert!(!deliver(&mut sink, &level, &[0.5; 960], now));
    }

    #[test]
    fn the_speaker_side_plays_the_queue_and_silence_when_poisoned() {
        let play = std::sync::Arc::new(Mutex::new(Playback::new(RATE)));
        play.lock().unwrap().push(1, &[0.5; 100]);
        let mut out = [9.0f32; 240];
        fill(&play, &mut out);
        assert!(out[..100].iter().all(|&x| x == 0.5));
        assert!(out[100..].iter().all(|&x| x == 0.0));
        let p = play.clone();
        let _ = std::thread::spawn(move || {
            let _g = p.lock().unwrap();
            panic!("poison");
        })
        .join();
        let mut out = [9.0f32; 16];
        fill(&play, &mut out);
        assert!(out.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn the_speaker_takes_only_the_frames_the_unit_plays() {
        // a 480-sample buffer for 240 frames: 240 come off the queue,
        // the next 240 stay for the next call (none skipped unheard)
        let play = Mutex::new(Playback::new(RATE));
        let pcm: Vec<f32> = (0..600).map(|i| i as f32 / 1000.0).collect();
        play.lock().unwrap().push(1, &pcm);
        play.lock().unwrap().end(1);
        let mut heard = Vec::new();
        for _ in 0..3 {
            let mut out = [9.0f32; 480];
            speaker_out(&play, &mut out, 240);
            assert!(out[240..].iter().all(|&x| x == 0.0));
            heard.extend_from_slice(&out[..240]);
        }
        assert_eq!(&heard[..600], &pcm[..]);
        assert!(play.lock().unwrap().done(1));
        // a frame count over the buffer: the buffer, no more
        let mut out = [9.0f32; 16];
        speaker_out(&play, &mut out, 64);
        assert!(out.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn statuses_read_as_their_four_letters_or_their_number() {
        assert_eq!(os_status(i32::from_be_bytes(*b"!pri")), "'!pri'");
        assert_eq!(os_status(-10875), "-10875");
        assert_eq!(check(0, "x"), Ok(()));
        assert_eq!(check(-50, "the mic's format"), Err("echo cancelling: the mic's format failed (-50)".into()));
    }

    #[test]
    fn aec_is_on_unless_turned_off() {
        assert!(wanted(None));
        assert!(wanted(Some("1")));
        for off in ["0", "off", "false", "no", " OFF "] {
            assert!(!wanted(Some(off)), "{}", off);
        }
    }

    #[test]
    fn the_voice_processing_unit_is_found_on_a_mac() {
        // a component lookup: no instance, no device, no sound
        assert_eq!(available(), cfg!(target_os = "macos"));
    }
}
