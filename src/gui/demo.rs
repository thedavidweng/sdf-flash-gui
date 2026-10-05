//! Browser demo host: the real GUI over a simulated `sdftool` (ADR 0011).
//!
//! [`DemoRunner`] answers the same `-l` / `--info` commands the desktop app
//! sends, with canned output that flows through the real parsers, workers, and
//! start gate. Commands that would touch a drive are refused.

use crate::command::Backend;
use crate::process::{CommandOutput, CommandRunOutcome, OperationControl, ProcessRunner};

use super::state::{AppState, Host};

use eframe::egui;

/// Tool path shown in Settings while the simulated backend is active.
pub const DEMO_TOOL_PATH: &str = "sdftool";

const DRIVE_LIST: &str = "\
Found 3 drives(s)
00: /dev/sr0
  HL-DT-ST_BD-RE_BU40N_1.03_211810241520_MODJ9TK3546
01: /dev/sr1
  ASUS_BW-16D1HT_3.10_212005070917_SIK04NAG90506
02: /dev/sr2
  HL-DT-ST_BD-RE_BH10LS30_1.00_200911051200_K9PG4C21337
";

const INFO_BU40N: &str = "\
SDF.bin version: 0x00A6
Vendor: HL-DT-ST
Product: BD-RE BU40N
Revision: 1.03
Drive ID: HL-DT-ST_BD-RE_BU40N_1.03_211810241520_MODJ9TK3546
Drive Specific SDF not present
Identification SDF present
8000:LibreDrive Information
8013:Status
8102:Possible, not yet enabled
8001:Drive platform
:MT1959
internal: mtk:19:59: H
";

const INFO_BW16D1HT: &str = "\
SDF.bin version: 0x00A6
Vendor: ASUS
Product: BW-16D1HT
Revision: 3.10
Drive ID: ASUS_BW-16D1HT_3.10_212005070917_SIK04NAG90506
Drive Specific SDF present
8001:Drive platform
:MT1959
internal: mtk:19:59: H
";

const INFO_BH10LS30: &str = "\
SDF.bin version: 0x00A6
Vendor: HL-DT-ST
Product: BD-RE BH10LS30
Revision: 1.00
Drive ID: HL-DT-ST_BD-RE_BH10LS30_1.00_200911051200_K9PG4C21337
Drive Specific SDF not present
LibreDrive Information
8001:Drive platform
:MT1939
";

/// Switch `state` to the web demo host with the simulated backend configured.
pub fn enter_web_demo(state: &mut AppState) {
    state.chrome.host = Host::WebDemo;
    state.config.backend = Backend::SdfTool;
    state.config.tool_path = DEMO_TOOL_PATH.into();
    state.config.sdf_path.clear();
    state.config.auto_detected = true;
    state.config.tool_detect_failed = false;
    state.config.sdf_detect_failed = false;
}

/// [`ProcessRunner`] that replays canned backend output and asks egui to repaint,
/// because the web build runs worker jobs inline during a frame.
pub struct DemoRunner {
    ctx: egui::Context,
}

impl DemoRunner {
    pub fn new(ctx: egui::Context) -> Self {
        Self { ctx }
    }
}

fn simulated_output(args: &[String]) -> Result<&'static str, String> {
    if args.iter().any(|a| a == "-l") {
        return Ok(DRIVE_LIST);
    }
    let device = args
        .iter()
        .position(|a| a == "-d")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str);
    match (device, args.iter().any(|a| a == "--info")) {
        (Some("/dev/sr0"), true) => Ok(INFO_BU40N),
        (Some("/dev/sr1"), true) => Ok(INFO_BW16D1HT),
        (Some("/dev/sr2"), true) => Ok(INFO_BH10LS30),
        _ => Err(unsupported(args)),
    }
}

fn unsupported(args: &[String]) -> String {
    format!(
        "web demo: `{DEMO_TOOL_PATH} {}` needs the desktop app",
        args.join(" ")
    )
}

impl ProcessRunner for DemoRunner {
    fn run_command(
        &self,
        _program: &str,
        args: &[String],
        _control: Option<&OperationControl>,
    ) -> Result<CommandRunOutcome, String> {
        self.ctx.request_repaint();
        let stdout = simulated_output(args)?;
        Ok(CommandRunOutcome::Completed(CommandOutput {
            status: std::process::ExitStatus::default(),
            stdout: stdout.into(),
            stderr: String::new(),
        }))
    }

    fn run_command_streaming(
        &self,
        _program: &str,
        args: &[String],
        _on_line: &dyn Fn(&str),
        _control: Option<&OperationControl>,
    ) -> Result<CommandRunOutcome, String> {
        self.ctx.request_repaint();
        Err(unsupported(args))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{plan_drive_info, plan_drive_list};
    use crate::drive::LibreDriveStatus;
    use crate::gui::workers::{poll_worker, spawn_list_drives, spawn_probe, WorkerMsg};
    use crate::gui::{ops, OperationMode};
    use std::sync::{mpsc, Arc};
    use std::time::{Duration, Instant};

    fn demo_state() -> AppState {
        let mut state = AppState::new_no_backend();
        enter_web_demo(&mut state);
        state
    }

    fn runner() -> Arc<dyn ProcessRunner> {
        Arc::new(DemoRunner::new(egui::Context::default()))
    }

    fn run(args: Vec<String>) -> Result<CommandRunOutcome, String> {
        DemoRunner::new(egui::Context::default()).run_command(DEMO_TOOL_PATH, &args, None)
    }

    fn completed(outcome: CommandRunOutcome) -> Option<CommandOutput> {
        match outcome {
            CommandRunOutcome::Completed(out) => Some(out),
            CommandRunOutcome::Cancelled | CommandRunOutcome::NeedsForceKill => None,
        }
    }

    fn stdout_of(outcome: CommandRunOutcome) -> String {
        let out = completed(outcome).expect("expected completed outcome");
        assert!(out.success());
        out.stdout
    }

    #[test]
    fn completed_helper_rejects_interrupted_outcomes() {
        assert!(completed(CommandRunOutcome::Cancelled).is_none());
        assert!(completed(CommandRunOutcome::NeedsForceKill).is_none());
    }

    fn pump_until(
        state: &mut AppState,
        rx: &mpsc::Receiver<WorkerMsg>,
        done: impl Fn(&AppState) -> bool,
    ) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done(state) && Instant::now() < deadline {
            poll_worker(state, rx, None);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(done(state), "worker did not finish in time");
    }

    fn probed(state: &AppState, drive: usize) -> bool {
        state.drive.last_probed_drive == Some(drive) && state.drive.drive_probed
    }

    #[test]
    fn enter_web_demo_configures_simulated_backend() {
        let state = demo_state();
        assert_eq!(state.chrome.host, Host::WebDemo);
        assert_eq!(state.config.tool_path, DEMO_TOOL_PATH);
        assert_eq!(state.config.backend, Backend::SdfTool);
        assert!(state.config.sdf_path.is_empty());
        assert!(state.config.auto_detected);
        assert!(ops::backend_configured(&state));
        assert!(!ops::system_access(&state));
        assert_eq!(
            ops::tool_path_status(&state, state.chrome.resolved_lang),
            Ok(())
        );
    }

    #[test]
    fn desktop_host_keeps_system_access_and_real_validation() {
        let state = AppState::new_no_backend();
        assert_eq!(state.chrome.host, Host::Desktop);
        assert!(ops::system_access(&state));
        assert!(!ops::backend_configured(&state));
        assert!(ops::tool_path_status(&state, state.chrome.resolved_lang).is_err());
    }

    #[test]
    fn list_and_info_answer_for_both_backends() {
        for backend in [Backend::SdfTool, Backend::MakeMkvCon] {
            let list = stdout_of(run(plan_drive_list(backend, DEMO_TOOL_PATH).args).unwrap());
            assert_eq!(list, DRIVE_LIST);
            for (device, info) in [
                ("/dev/sr0", INFO_BU40N),
                ("/dev/sr1", INFO_BW16D1HT),
                ("/dev/sr2", INFO_BH10LS30),
            ] {
                let cmd = plan_drive_info(backend, DEMO_TOOL_PATH, device);
                assert_eq!(stdout_of(run(cmd.args).unwrap()), info);
            }
        }
    }

    #[test]
    fn unknown_device_or_command_is_refused() {
        let unknown = plan_drive_info(Backend::SdfTool, DEMO_TOOL_PATH, "/dev/sr9");
        let err = run(unknown.args).err().unwrap();
        assert!(err.contains("/dev/sr9"));
        assert!(err.contains("desktop app"));

        let dump = vec![
            "-d".to_string(),
            "/dev/sr0".to_string(),
            "--dump".to_string(),
        ];
        assert!(run(dump).is_err());
        assert!(run(Vec::new()).is_err());
    }

    #[test]
    fn streaming_commands_are_refused() {
        let args = vec!["-d".to_string(), "/dev/sr0".to_string()];
        let err = DemoRunner::new(egui::Context::default())
            .run_command_streaming(DEMO_TOOL_PATH, &args, &|_| {}, None)
            .err()
            .unwrap();
        assert!(err.contains("-d /dev/sr0"));
    }

    #[test]
    fn simulated_drives_flow_through_real_list_and_probe_pipeline() {
        let mut state = demo_state();
        let runner = runner();
        let (tx, rx) = mpsc::channel();

        spawn_list_drives(&tx, &mut state, &runner, true);
        pump_until(&mut state, &rx, |s| !s.runtime.busy);
        assert_eq!(state.drive.drives.len(), 3);
        assert_eq!(state.drive.selected_drive, Some(0));
        assert_eq!(state.drive.drives[0].product, "BD-RE BU40N");
        assert_eq!(state.drive.drives[1].vendor, "ASUS");

        spawn_probe(&tx, &mut state, 0, &runner);
        pump_until(&mut state, &rx, |s| probed(s, 0));
        assert!(state.drive.drive_probed);
        assert!(state.drive.drive_mt1959);
        assert!(!state.drive.drive_encrypted_firmware);
        assert_eq!(
            state.drive.drive_libredrive,
            LibreDriveStatus::PossibleNotEnabled
        );
        assert_eq!(state.drive.drive_sdf_version.as_deref(), Some("0x00A6"));

        state.drive.selected_drive = Some(1);
        spawn_probe(&tx, &mut state, 1, &runner);
        pump_until(&mut state, &rx, |s| probed(s, 1));
        assert!(state.drive.drive_mt1959);
        assert!(state.drive.drive_encrypted_firmware);
        assert_eq!(state.drive.drive_libredrive, LibreDriveStatus::Enabled);

        state.drive.selected_drive = Some(2);
        spawn_probe(&tx, &mut state, 2, &runner);
        pump_until(&mut state, &rx, |s| probed(s, 2));
        assert!(!state.drive.drive_mt1959);
        assert!(state.drive.drive_mt1939);
    }

    #[test]
    fn start_stays_disabled_with_web_demo_reason() {
        let mut state = demo_state();
        let runner = runner();
        let (tx, rx) = mpsc::channel();
        spawn_list_drives(&tx, &mut state, &runner, false);
        pump_until(&mut state, &rx, |s| !s.runtime.busy);
        spawn_probe(&tx, &mut state, 0, &runner);
        pump_until(&mut state, &rx, |s| probed(s, 0));

        state.operation_mode = OperationMode::Read;
        assert!(!ops::can_start(&state));
        assert_eq!(
            ops::start_disabled_reason(&state),
            crate::i18n::t(
                crate::i18n::L10nKey::ReasonWebDemo,
                state.chrome.resolved_lang
            )
        );
    }
}
