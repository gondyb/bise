//! Fake ports for the voice tests: no microphone, no network.

use super::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Mutex;

pub(crate) struct FakeCapture {
    pub(crate) peak: f32,
    pub(crate) signal: bool,
    pub(crate) stopped: Rc<RefCell<bool>>,
}

impl Capture for FakeCapture {
    fn peak(&self) -> f32 {
        self.peak
    }
    fn has_signal(&self) -> bool {
        self.signal
    }
}

impl Drop for FakeCapture {
    fn drop(&mut self) {
        *self.stopped.borrow_mut() = true;
    }
}

#[derive(Clone)]
pub(crate) struct FakeRecorder {
    pub(crate) result: Result<(), StartError>,
    pub(crate) signal: bool,
    pub(crate) stopped: Rc<RefCell<bool>>,
    pub(crate) audio: Rc<RefCell<Option<Sender<AudioMsg>>>>,
}

impl FakeRecorder {
    pub(crate) fn ok(signal: bool) -> Self {
        FakeRecorder {
            result: Ok(()),
            signal,
            stopped: Rc::new(RefCell::new(false)),
            audio: Rc::new(RefCell::new(None)),
        }
    }
}

impl Recorder for FakeRecorder {
    fn start(&mut self, sample_rate: u32, audio: Sender<AudioMsg>) -> Result<Box<dyn Capture>, StartError> {
        assert_eq!(sample_rate, 16_000);
        self.result.clone()?;
        *self.audio.borrow_mut() = Some(audio);
        *self.stopped.borrow_mut() = false;
        Ok(Box::new(FakeCapture { peak: 0.5, signal: self.signal, stopped: self.stopped.clone() }))
    }
}

/// The ends of a started session: the audio it receives, the event
/// sender, the cancel flag, the API key.
pub(crate) type Session = (Receiver<AudioMsg>, Sender<TranscribeEvent>, Arc<AtomicBool>, String);

/// Hands the test the session's ends.
#[derive(Clone, Default)]
pub(crate) struct FakeTranscriber {
    pub(crate) session: Arc<Mutex<Option<Session>>>,
}

impl Transcriber for FakeTranscriber {
    fn start(&self, api_key: String, audio: Receiver<AudioMsg>, events: Sender<TranscribeEvent>, cancel: Arc<AtomicBool>) {
        *self.session.lock().unwrap() = Some((audio, events, cancel, api_key));
    }
}

impl FakeTranscriber {
    pub(crate) fn send(&self, ev: TranscribeEvent) {
        let s = self.session.lock().unwrap();
        s.as_ref().unwrap().1.send(ev).unwrap();
    }
    pub(crate) fn close(&self) {
        let mut s = self.session.lock().unwrap();
        if let Some((a, _, c, k)) = s.take() {
            // drop the event sender: the session thread is gone
            *s = Some((a, mpsc::channel().0, c, k));
        }
    }
    pub(crate) fn cancelled(&self) -> bool {
        self.session.lock().unwrap().as_ref().unwrap().2.load(Ordering::SeqCst)
    }
    pub(crate) fn audio(&self) -> Vec<AudioMsg> {
        let s = self.session.lock().unwrap();
        s.as_ref().unwrap().0.try_iter().collect()
    }
}

