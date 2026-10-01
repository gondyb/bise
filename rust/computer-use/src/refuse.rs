//! The hard refusals (design §5.1, §5.2): targets no agent drives, in any
//! approval mode. They are refusals, not approvals: `yolo` does not lift
//! them. The extension and the helper check them too (defence in depth);
//! this is the first check. Each `Some` is the C1 `refused` message.

use crate::proto::host_of;

/// Browser-internal schemes: settings, extensions, the debugger.
const SCHEMES: [&str; 11] = [
    "chrome:", "chrome-extension:", "chrome-untrusted:", "devtools:", "edge:", "brave:", "vivaldi:", "opera:",
    "arc:", "view-source:", "chrome-search:",
];

/// The extension stores (an agent must not install or remove extensions).
const STORES: [(&str, &str); 4] = [
    ("chromewebstore.google.com", ""),
    ("chrome.google.com", "/webstore"),
    ("microsoftedge.microsoft.com", "/addons"),
    ("addons.opera.com", ""),
];

/// A URL no agent opens or acts on.
pub fn url(url: &str) -> Option<String> {
    let u = url.trim().to_ascii_lowercase();
    if let Some(s) = SCHEMES.iter().find(|s| u.starts_with(*s)) {
        return Some(format!(
            "{}// pages are the browser's own settings and are off limits; ask the user to do this step",
            s.trim_end_matches(':').to_string() + ":"
        ));
    }
    if u.starts_with("about:") && u != "about:blank" {
        return Some("about: pages are off limits; ask the user to do this step".into());
    }
    let host = host_of(&u);
    let path = u.split_once("://").map(|(_, r)| r).unwrap_or("");
    let path = path.find('/').map(|i| &path[i..]).unwrap_or("");
    for (h, p) in STORES {
        if host == h && path.starts_with(p) {
            return Some("extension stores are off limits; ask the user to install or remove extensions".into());
        }
    }
    None
}

/// An action the user does himself on this page: the password step of a
/// Google sign-in (design §5.1). Typing only; reading and closing stay.
pub fn typing(url: &str, action: &str) -> Option<String> {
    let u = url.to_ascii_lowercase();
    let typing = matches!(action, "fill" | "type" | "press");
    let pwd = host_of(&u) == "accounts.google.com" && (u.contains("/challenge/pwd") || u.contains("/signin/v2/challenge/pw"));
    (typing && pwd).then(|| "the user types his Google password himself; ask him to sign in, then go on".into())
}

/// Bundle ids of apps no agent drives: terminals, bise, password
/// managers, Keychain Access, the login window.
const APPS: [&str; 22] = [
    "com.apple.Terminal",
    "com.mitchellh.ghostty",
    "com.googlecode.iterm2",
    "com.github.wez.wezterm",
    "net.kovidgoyal.kitty",
    "org.alacritty",
    "io.alacritty",
    "dev.warp.Warp-Stable",
    "dev.warp.Warp",
    "co.zeit.hyper",
    "dev.bise.computer-use",
    "com.apple.keychainaccess",
    "com.apple.Passwords",
    "com.1password.1password",
    "com.agilebits.onepassword7",
    "com.agilebits.onepassword-osx",
    "com.bitwarden.desktop",
    "com.lastpass.LastPass",
    "com.dashlane.dashlanephonefinal",
    "com.apple.loginwindow",
    "com.apple.SecurityAgent",
    "com.apple.settings.PrivacySecurity.extension",
];

/// An app (by bundle id) no agent drives; `title`: the window it is on,
/// when known (System Settings is fine, its Privacy & Security pane is not).
pub fn app(bundle: &str, title: Option<&str>) -> Option<String> {
    let b = bundle.to_ascii_lowercase();
    if APPS.iter().any(|a| a.to_ascii_lowercase() == b) || b.starts_with("dev.bise.") {
        return Some(format!("{} is off limits to agents (terminals, bise, passwords, login); ask the user", bundle));
    }
    let settings = b == "com.apple.systempreferences" || b == "com.apple.settings";
    if settings && title.is_some_and(|t| t.to_ascii_lowercase().contains("privacy")) {
        return Some("Privacy & Security settings are off limits; ask the user to change them".into());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_refusals() {
        for u in [
            "chrome://settings",
            "CHROME://extensions",
            "chrome-extension://abc/page.html",
            "edge://settings",
            "about:config",
            "https://chromewebstore.google.com/detail/x",
            "https://chrome.google.com/webstore/detail/x",
            "https://microsoftedge.microsoft.com/addons/detail/x",
        ] {
            assert!(url(u).is_some(), "{}", u);
        }
        for u in ["https://www.amazon.fr/", "about:blank", "http://127.0.0.1:8080/", "https://chrome.google.com/intl/fr/"] {
            assert!(url(u).is_none(), "{}", u);
        }
        let pwd = "https://accounts.google.com/v3/signin/challenge/pwd?x=1";
        assert!(typing(pwd, "fill").is_some());
        assert!(typing(pwd, "click").is_none());
        assert!(typing("https://accounts.google.com/v3/signin/identifier", "fill").is_none());
    }

    #[test]
    fn app_refusals() {
        for b in ["com.apple.Terminal", "com.mitchellh.ghostty", "com.1password.1password", "dev.bise.anything", "com.apple.loginwindow"] {
            assert!(app(b, None).is_some(), "{}", b);
        }
        assert!(app("com.apple.TextEdit", None).is_none());
        assert!(app("com.apple.systempreferences", Some("Displays")).is_none());
        assert!(app("com.apple.systempreferences", Some("Privacy & Security")).is_some());
    }
}
