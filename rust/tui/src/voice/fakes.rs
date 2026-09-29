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
