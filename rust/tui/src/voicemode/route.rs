//! Headphones or speakers (owner: voice-audio; plan §2, §4.2): with
//! headphones you cut in by talking; on speakers the mic waits while the
//! agent talks (it would hear itself).
//!
//! macOS: the default output device's transport type, data source and
//! name, read from CoreAudio (`AudioObjectGetPropertyData`; reading a
//! property opens nothing and plays nothing), then [`classify`] (pure).
//! Elsewhere: Unknown (asked once, then saved).

use super::Route;

/// A CoreAudio four-char code.
const fn fourcc(s: &[u8; 4]) -> u32 {
    ((s[0] as u32) << 24) | ((s[1] as u32) << 16) | ((s[2] as u32) << 8) | s[3] as u32
}

/// kAudioDeviceTransportType*
pub mod transport {
    use super::fourcc;
    pub const BUILT_IN: u32 = fourcc(b"bltn");
    pub const BLUETOOTH: u32 = fourcc(b"blue");
    pub const BLUETOOTH_LE: u32 = fourcc(b"blea");
    pub const USB: u32 = fourcc(b"usb ");
    pub const HDMI: u32 = fourcc(b"hdmi");
    pub const DISPLAY_PORT: u32 = fourcc(b"dprt");
    pub const AIRPLAY: u32 = fourcc(b"airp");
    pub const VIRTUAL: u32 = fourcc(b"virt");
    pub const AGGREGATE: u32 = fourcc(b"grup");
    pub const THUNDERBOLT: u32 = fourcc(b"thun");
}

/// kAudioDevicePropertyDataSource values of the built-in output.
pub mod source {
    use super::fourcc;
    /// the internal speakers
    pub const INTERNAL_SPEAKER: u32 = fourcc(b"ispk");
    /// the headphone jack
    pub const HEADPHONES: u32 = fourcc(b"hdpn");
}

/// The route of an output device from what CoreAudio says of it.
/// `name`: the device's name ("MacBook Pro Speakers", "AirPods Pro").
/// Unsure stays Unknown: the user is asked once, which beats an agent
/// that talks over itself on a speaker read as headphones (a Bluetooth
/// device is Unknown unless its name says headset or speaker).
pub fn classify(transport: u32, source: Option<u32>, name: &str) -> Route {
    let name = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
    let speaker_name = has(&["speaker", "soundbar", "homepod", "sonos", "soundlink", "boom", "pill", "enceinte"]);
    let headset_name = has(&[
        "headphone", "headset", "earphone", "airpods", "earpods", "earbuds", "buds", "beats", "casque", "écouteurs",
        "quietcomfort", "wh-", "wf-",
    ]);
    match transport {
        transport::BUILT_IN => match source {
            Some(source::HEADPHONES) => Route::Headphones,
            Some(source::INTERNAL_SPEAKER) => Route::Speakers,
            // Apple silicon: the jack is its own device, "External Headphones"
            _ if headset_name => Route::Headphones,
            _ => Route::Speakers,
        },
        // Bluetooth is headphones or a speaker, and only the name tells:
        // a speaker by its name, a headset by its name, else ask (round 2:
        // the user's speaker "Bose Mini Gabriel" read as headphones, so
        // the agent talked over its own voice)
        transport::BLUETOOTH | transport::BLUETOOTH_LE if speaker_name => Route::Speakers,
        transport::BLUETOOTH | transport::BLUETOOTH_LE if headset_name => Route::Headphones,
        transport::BLUETOOTH | transport::BLUETOOTH_LE => Route::Unknown,
        // a USB headset says so; a USB DAC to monitors does not
        transport::USB | transport::THUNDERBOLT if headset_name => Route::Headphones,
        transport::USB | transport::THUNDERBOLT if speaker_name => Route::Speakers,
        // a display's or a TV's speakers, a room's AirPlay speaker
        transport::HDMI | transport::DISPLAY_PORT | transport::AIRPLAY => Route::Speakers,
        _ if headset_name => Route::Headphones,
        _ if speaker_name => Route::Speakers,
        _ => Route::Unknown,
    }
}

/// Where the voice comes out now (cheap: three property reads; call it
/// when voice mode opens and again when you want to follow a change).
pub fn output_route() -> Route {
    #[cfg(target_os = "macos")]
    {
        match mac::default_output() {
            Some(d) => classify(d.transport, d.source, &d.name),
            None => Route::Unknown,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Route::Unknown
    }
}

/// The default output device in one line, for the debug log
/// (`"MacBook Pro Speakers" · bltn · ispk`).
pub fn output_line() -> String {
    #[cfg(target_os = "macos")]
    {
        let code = |c: u32| String::from_utf8_lossy(&c.to_be_bytes()).into_owned();
        match mac::default_output() {
            Some(d) => format!("\"{}\" · {} · {}", d.name, code(d.transport), d.source.map_or("-".into(), code)),
            None => "no default output".into(),
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        "unknown".into()
    }
}

#[cfg(target_os = "macos")]
mod mac {
    //! The few CoreAudio calls the route needs (the frameworks are
    //! linked by cpal already; no new crate).

    use super::fourcc;
    use std::ffi::c_void;

    #[repr(C)]
    struct PropertyAddress {
        selector: u32,
        scope: u32,
        element: u32,
    }

    const SYSTEM_OBJECT: u32 = 1;
    const DEFAULT_OUTPUT_DEVICE: u32 = fourcc(b"dOut");
    const TRANSPORT_TYPE: u32 = fourcc(b"tran");
    const DATA_SOURCE: u32 = fourcc(b"ssrc");
    const NAME: u32 = fourcc(b"lnam");
    const SCOPE_GLOBAL: u32 = fourcc(b"glob");
    const SCOPE_OUTPUT: u32 = fourcc(b"outp");
    const ELEMENT_MAIN: u32 = 0;
    const UTF8: u32 = 0x0800_0100; // kCFStringEncodingUTF8

    #[link(name = "CoreAudio", kind = "framework")]
    extern "C" {
        fn AudioObjectGetPropertyData(
            object: u32,
            address: *const PropertyAddress,
            qualifier_size: u32,
            qualifier: *const c_void,
            size: *mut u32,
            data: *mut c_void,
        ) -> i32;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringGetCString(s: *const c_void, buf: *mut u8, size: isize, encoding: u32) -> u8;
        fn CFRelease(cf: *const c_void);
    }

    pub struct Output {
        pub transport: u32,
        pub source: Option<u32>,
        pub name: String,
    }

    fn get<T: Copy>(object: u32, selector: u32, scope: u32, init: T) -> Option<T> {
        let address = PropertyAddress { selector, scope, element: ELEMENT_MAIN };
        let mut value = init;
        let mut size = std::mem::size_of::<T>() as u32;
        // SAFETY: `value` is a T of `size` bytes, the address lives for the call
        let status = unsafe {
            AudioObjectGetPropertyData(object, &address, 0, std::ptr::null(), &mut size, &mut value as *mut T as *mut c_void)
        };
        (status == 0 && size as usize == std::mem::size_of::<T>()).then_some(value)
    }

    fn name(device: u32) -> String {
        let Some(cf) = get::<*const c_void>(device, NAME, SCOPE_GLOBAL, std::ptr::null()) else { return String::new() };
        if cf.is_null() {
            return String::new();
        }
        let mut buf = [0u8; 256];
        // SAFETY: `cf` is the CFString the property call returned (we own
        // it: released once below); `buf` is 256 bytes
        let ok = unsafe { CFStringGetCString(cf, buf.as_mut_ptr(), buf.len() as isize, UTF8) } != 0;
        unsafe { CFRelease(cf) };
        if !ok {
            return String::new();
        }
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        String::from_utf8_lossy(&buf[..end]).into_owned()
    }

    pub fn default_output() -> Option<Output> {
        let device = get::<u32>(SYSTEM_OBJECT, DEFAULT_OUTPUT_DEVICE, SCOPE_GLOBAL, 0).filter(|&d| d != 0)?;
        let transport = get::<u32>(device, TRANSPORT_TYPE, SCOPE_GLOBAL, 0).unwrap_or(0);
        let source = get::<u32>(device, DATA_SOURCE, SCOPE_OUTPUT, 0);
        Some(Output { transport, source, name: name(device) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Route::*;

    #[test]
    fn the_built_in_output_follows_its_data_source() {
        assert_eq!(classify(transport::BUILT_IN, Some(source::INTERNAL_SPEAKER), "MacBook Pro Speakers"), Speakers);
        assert_eq!(classify(transport::BUILT_IN, Some(source::HEADPHONES), "Built-in Output"), Headphones);
        // Apple silicon: the jack is a device of its own
        assert_eq!(classify(transport::BUILT_IN, None, "External Headphones"), Headphones);
        // a Mac mini's or an iMac's own speakers, no data source
        assert_eq!(classify(transport::BUILT_IN, None, "Mac mini Speakers"), Speakers);
        assert_eq!(classify(transport::BUILT_IN, None, "Built-in Output"), Speakers);
    }

    #[test]
    fn bluetooth_says_what_it_is_by_its_name_or_stays_unknown() {
        assert_eq!(classify(transport::BLUETOOTH, None, "Gabriel's AirPods Pro"), Headphones);
        assert_eq!(classify(transport::BLUETOOTH, None, "WH-1000XM4"), Headphones);
        assert_eq!(classify(transport::BLUETOOTH, None, "Bose QuietComfort 45"), Headphones);
        assert_eq!(classify(transport::BLUETOOTH_LE, None, "Galaxy Buds2"), Headphones);
        assert_eq!(classify(transport::BLUETOOTH, None, "JBL Flip Speaker"), Speakers);
        assert_eq!(classify(transport::BLUETOOTH, None, "Bose SoundLink Mini"), Speakers);
        assert_eq!(classify(transport::BLUETOOTH, None, "Beats Pill"), Speakers);
        // the user's speaker, renamed: nothing in the name says which
        assert_eq!(classify(transport::BLUETOOTH, None, "Bose Mini Gabriel"), Unknown);
        assert_eq!(classify(transport::BLUETOOTH, None, "JBL Charge 5"), Unknown);
        assert_eq!(classify(transport::BLUETOOTH_LE, None, ""), Unknown);
    }

    #[test]
    fn usb_says_what_it_is_or_stays_unknown() {
        assert_eq!(classify(transport::USB, None, "Jabra Evolve2 Headset"), Headphones);
        assert_eq!(classify(transport::USB, None, "USB-C to 3.5mm Headphone Jack Adapter"), Headphones);
        assert_eq!(classify(transport::USB, None, "Logitech USB Speaker"), Speakers);
        // an audio interface to monitors, or to headphones: ask
        assert_eq!(classify(transport::USB, None, "Scarlett 2i2 USB"), Unknown);
    }

    #[test]
    fn displays_and_airplay_are_speakers_the_rest_unknown() {
        assert_eq!(classify(transport::HDMI, None, "LG TV"), Speakers);
        assert_eq!(classify(transport::DISPLAY_PORT, None, "Studio Display"), Speakers);
        assert_eq!(classify(transport::AIRPLAY, None, "Living Room"), Speakers);
        assert_eq!(classify(transport::VIRTUAL, None, "BlackHole 2ch"), Unknown);
        assert_eq!(classify(transport::AGGREGATE, None, "Multi-Output Device"), Unknown);
        assert_eq!(classify(0, None, ""), Unknown);
        assert_eq!(classify(transport::VIRTUAL, None, "Teams Headset"), Headphones);
    }

    #[test]
    fn the_four_char_codes_are_coreaudios() {
        assert_eq!(transport::BUILT_IN, 0x626c_746e);
        assert_eq!(transport::BLUETOOTH, 0x626c_7565);
        assert_eq!(transport::USB, 0x7573_6220);
        assert_eq!(source::HEADPHONES, 0x6864_706e);
        assert_eq!(source::INTERNAL_SPEAKER, 0x6973_706b);
    }

    #[test]
    fn the_route_reads_without_a_panic() {
        // reads the default output's properties: no stream, no sound
        let r = output_route();
        assert!(matches!(r, Headphones | Speakers | Unknown));
    }
}
