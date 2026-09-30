//! Fake ports for the voice tests: no microphone, no network.

use super::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Mutex;

pub(crate) struct FakeCapture {
    /// the real level and meter, fed by [`FakeRecorder::speak`]
    pub(crate) level: Arc<Level>,
    pub(crate) signal: bool,
    pub(crate) stopped: Rc<RefCell<bool>>,
}

impl Capture for FakeCapture {
    fn has_signal(&self) -> bool {
        self.signal
    }
    fn levels(&self) -> [f32; chip::BARS] {
        self.level.levels()
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
    /// the running capture's level (the audio thread's side)
    pub(crate) level: Rc<RefCell<Option<Arc<Level>>>>,
}

impl FakeRecorder {
    pub(crate) fn ok(signal: bool) -> Self {
        FakeRecorder {
            result: Ok(()),
            signal,
            stopped: Rc::new(RefCell::new(false)),
            audio: Rc::new(RefCell::new(None)),
            level: Rc::new(RefCell::new(None)),
        }
    }

    /// The microphone hears `blocks` meter blocks at `amp` (0..1 of full
    /// scale): what the audio thread does with each one (level, meter,
    /// then the transcriber), from outside the UI loop.
    pub(crate) fn speak(&self, amp: f32, blocks: usize) {
        let level = self.level.borrow().clone().expect("recording");
        let audio = self.audio.borrow().clone().expect("recording");
        for _ in 0..blocks {
            level.block(vec![to_i16(amp); chip::METER_BLOCK], &audio);
        }
    }
}

impl Recorder for FakeRecorder {
    fn start(&mut self, sample_rate: u32, audio: Sender<AudioMsg>) -> Result<Box<dyn Capture>, StartError> {
        assert_eq!(sample_rate, 16_000);
        self.result.clone()?;
        *self.audio.borrow_mut() = Some(audio);
        *self.stopped.borrow_mut() = false;
        let level = Arc::new(Level::default());
        *self.level.borrow_mut() = Some(level.clone());
        Ok(Box::new(FakeCapture { level, signal: self.signal, stopped: self.stopped.clone() }))
    }
}

/// The ends of a started session: the audio it receives, the event
/// sender, the cancel flag, the voice job.
pub(crate) type Session = (Receiver<AudioMsg>, Sender<TranscribeEvent>, Arc<AtomicBool>, VoiceJob);

/// A Mistral job with a fake key.
pub(crate) fn job() -> VoiceJob {
    VoiceJob {
        name: "mistral/voxtral-mini-latest".into(),
        api: "mistral".into(),
        base_url: "https://api.mistral.ai/v1".into(),
        model: "voxtral-mini-latest".into(),
        key: "sk-test".into(),
        language: None,
        vocabulary: Vec::new(),
    }
}

/// Hands the test the session's ends.
#[derive(Clone, Default)]
pub(crate) struct FakeTranscriber {
    pub(crate) session: Arc<Mutex<Option<Session>>>,
}

impl Transcriber for FakeTranscriber {
    fn start(&self, job: VoiceJob, audio: Receiver<AudioMsg>, events: Sender<TranscribeEvent>, cancel: Arc<AtomicBool>) {
        *self.session.lock().unwrap() = Some((audio, events, cancel, job));
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


/// A one-shot HTTP server on 127.0.0.1: reads one request (head +
/// Content-Length body), answers `status` with `body`, hands the raw
/// request to the test. Returns its base URL (".../v1") and the request.
pub(crate) fn serve_once(status: u16, body: &str) -> (String, Receiver<Vec<u8>>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    let body = body.to_string();
    std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut req = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            let n = s.read(&mut buf).unwrap();
            req.extend_from_slice(&buf[..n]);
            if let Some(end) = req.windows(4).position(|w| w == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&req[..end]).to_lowercase();
                let len = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
                    .unwrap_or(0);
                if req.len() >= end + 4 + len || n == 0 {
                    break;
                }
            }
        }
        // chunked, to test the client's decoding
        let reply = format!(
            "HTTP/1.1 {} X\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n",
            status,
            body.len(),
            body
        );
        s.write_all(reply.as_bytes()).unwrap();
        let _ = tx.send(req);
    });
    (url, rx)
}
