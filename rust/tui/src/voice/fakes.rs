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


/// A scripted server: what the loop sent, and the replies to give. The
/// server answers `transcription.done` only after it got flush + end;
/// until then (and after the script) it is idle.
#[derive(Default)]
pub(crate) struct FakeSocket {
    pub(crate) sent: Vec<String>,
    /// replies given as soon as asked (deltas while recording)
    pub(crate) script: std::collections::VecDeque<SocketRead>,
    /// replies given once flush + end arrived
    pub(crate) after_end: std::collections::VecDeque<SocketRead>,
    pub(crate) closed: bool,
    pub(crate) reads: usize,
}

impl FakeSocket {
    /// The `type` of each sent message.
    pub(crate) fn sent_types(&self) -> Vec<String> {
        self.sent
            .iter()
            .map(|m| {
                let v: Value = serde_json::from_str(m).unwrap();
                v["type"].as_str().unwrap().to_string()
            })
            .collect()
    }
    fn got_end(&self) -> bool {
        let t = self.sent_types();
        t.iter().any(|x| x == "input_audio.flush") && t.iter().any(|x| x == "input_audio.end")
    }
}

impl RealtimeSocket for FakeSocket {
    fn send_text(&mut self, text: String) -> Result<(), String> {
        self.sent.push(text);
        Ok(())
    }
    fn read(&mut self, _wait: Duration) -> Result<SocketRead, String> {
        self.reads += 1;
        // a stuck loop fails the test instead of hanging it
        assert!(self.reads < 10_000, "the session loop never ended");
        if let Some(r) = self.script.pop_front() {
            return Ok(r);
        }
        if self.got_end() {
            if let Some(r) = self.after_end.pop_front() {
                return Ok(r);
            }
        }
        Ok(SocketRead::Idle)
    }
    fn close(&mut self) {
        self.closed = true;
    }
}

pub(crate) fn server_text(v: Value) -> SocketRead {
    SocketRead::Text(v.to_string())
}
