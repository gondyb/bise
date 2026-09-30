//! What the Bend REPL announces at startup (its `harness-info` line),
//! read by `--headless` for its READY line.

/// The `harness-info` line of the REPL:
/// the REPL is the single source of truth, the TUI never recomputes
/// the model, the threshold or the side-channel paths.
#[derive(Clone, Debug, Default)]
pub(crate) struct HarnessInfo {
    /// "" = no model yet (BISE-266: none built in); the REPL still
    /// starts, its first turn says how to pick one (BISE-280)
    pub(crate) model: String,
    pub(crate) threshold: String,
    pub(crate) steer_path: String,
    pub(crate) interrupt_path: String,
}

impl HarnessInfo {
    /// Parse `harness-info model=M threshold=N steer=P interrupt=Q`;
    /// M may be empty (no model yet), the two paths may not.
    pub(crate) fn parse(line: &str) -> Option<HarnessInfo> {
        let rest = line.trim().strip_prefix("harness-info ")?;
        let mut info = HarnessInfo::default();
        for kv in rest.split_whitespace() {
            let (k, v) = kv.split_once('=')?;
            match k {
                "model" => info.model = v.to_string(),
                "threshold" => info.threshold = v.to_string(),
                "steer" => info.steer_path = v.to_string(),
                "interrupt" => info.interrupt_path = v.to_string(),
                _ => {}
            }
        }
        if info.steer_path.is_empty() || info.interrupt_path.is_empty() {
            return None;
        }
        Some(info)
    }

    /// Find the line in a REPL log.
    pub(crate) fn from_log(log: &str) -> Option<HarnessInfo> {
        log.lines().find_map(HarnessInfo::parse)
    }
}

#[cfg(test)]
mod harness_info_tests {
    use super::HarnessInfo;

    // the exact string LAWS.bend pins for Rt.info_line (law
    // info_line_format): the two sides of the contract agree
    const BEND_LINE: &str = "harness-info model=claude-opus-5-5 threshold=800000 steer=/tmp/bend-steer-7.txt interrupt=/tmp/bend-interrupt-7.txt";

    #[test]
    fn parses_the_line_bend_prints() {
        let info = HarnessInfo::parse(BEND_LINE).expect("parses");
        assert_eq!(info.model, "claude-opus-5-5");
        assert_eq!(info.threshold, "800000");
        assert_eq!(info.steer_path, "/tmp/bend-steer-7.txt");
        assert_eq!(info.interrupt_path, "/tmp/bend-interrupt-7.txt");
    }

    #[test]
    fn finds_the_line_in_a_repl_log() {
        let log = format!("{}\nbend-harness LIVE REPL on 127.0.0.1:7 ...\n[mcp] connector index written\n", BEND_LINE);
        assert_eq!(HarnessInfo::from_log(&log).expect("found").model, "claude-opus-5-5");
    }

    #[test]
    fn rejects_an_incomplete_line() {
        assert!(HarnessInfo::parse("harness-info model=m threshold=1").is_none());
        assert!(HarnessInfo::parse("bend-harness LIVE REPL on 127.0.0.1:7").is_none());
    }

    // BISE-280: a fresh install has no model (BISE-266); the REPL
    // announces `model=` and must still start
    #[test]
    fn accepts_no_model_yet() {
        let info = HarnessInfo::parse(
            "harness-info model= threshold=102400 steer=/tmp/bend-steer-7.txt interrupt=/tmp/bend-interrupt-7.txt",
        )
        .expect("parses");
        assert_eq!(info.model, "");
        assert_eq!(info.steer_path, "/tmp/bend-steer-7.txt");
    }
}

