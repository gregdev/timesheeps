//! Windows scheduled-task watchdog: relaunch Timesheeps if it is not running.
//!
//! # Why this exists
//!
//! `tauri-plugin-autostart` writes an `HKCU\...\Run` value, which Windows
//! evaluates **exactly once per logon** and never retries. On 2026-11-18 the app
//! was launched by that Run key at 12:04, created its WebView2 profile at 12:08,
//! and then exited without leaving any trace. Nothing relaunched it, and because
//! the laptop was only ever woken from Modern Standby (never rebooted or logged
//! off again) the Run key was not evaluated for another **eight days**. Five
//! working days of activity were never recorded.
//!
//! A logon-only trigger cannot recover from that, so on Windows we additionally
//! register a scheduled task that starts the app every [`REPEAT_MINUTES`] while
//! the user is logged on, plus one run shortly after logon (replacing the Run
//! key's one-shot behaviour with something that can retry).
//!
//! # Why repeating is safe
//!
//! `tauri-plugin-single-instance` is registered *first* in `lib.rs`, so an
//! invocation while the app is healthy hands its arguments to the running copy
//! and exits immediately. The fifteen-minute tick therefore costs one short-lived
//! process and changes nothing. It also means the task never fights the tray
//! icon or the poller for the database.
//!
//! The task is created with `LeastPrivilege` / `InteractiveToken`, so no
//! elevation is required and it only runs while the user is logged on — which is
//! precisely the window in which tracking is meaningful.

use tauri::AppHandle;

/// Name of the scheduled task, as shown in Task Scheduler.
pub const TASK_NAME: &str = "Timesheeps Watchdog";

/// How often the task re-launches the app, in minutes.
const REPEAT_MINUTES: u32 = 15;

/// Ensure the watchdog task matches `enabled`.
///
/// Never returns an error: the Run key is the primary autostart mechanism, so a
/// refusal from Task Scheduler (group policy, locked-down machine) degrades to
/// the old behaviour and is only logged.
pub fn sync(app: &AppHandle, enabled: bool) {
    let _ = app; // only used on Windows; keeps the signature uniform
    match sync_inner(enabled) {
        Ok(()) => {}
        Err(e) => log::warn!("watchdog: {e}"),
    }
}

#[cfg(not(windows))]
fn sync_inner(_enabled: bool) -> Result<(), String> {
    // No scheduled-task equivalent is worth the platform-specific code here:
    // macOS and Linux are development targets for this app.
    Ok(())
}

#[cfg(windows)]
fn sync_inner(enabled: bool) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use windows::Win32::System::Threading::CREATE_NO_WINDOW;

    /// Run `schtasks` without flashing a console window.
    fn schtasks(args: &[&str]) -> Result<std::process::Output, String> {
        std::process::Command::new("schtasks")
            .args(args)
            .creation_flags(CREATE_NO_WINDOW.0)
            .output()
            .map_err(|e| format!("could not run schtasks: {e}"))
    }

    fn task_exists() -> bool {
        schtasks(&["/Query", "/TN", TASK_NAME])
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    if !enabled {
        if task_exists() {
            match schtasks(&["/Delete", "/TN", TASK_NAME, "/F"]) {
                Ok(out) if out.status.success() => {
                    log::info!("watchdog: scheduled task removed");
                }
                Ok(out) => log::warn!(
                    "watchdog: could not delete the task: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
                Err(e) => log::warn!("watchdog: {e}"),
            }
        }
        return Ok(());
    }

    // Pointing a scheduled task at a build output that the next `cargo build`
    // replaces would leave a task aiming at a stale path, so debug builds opt in
    // explicitly (`TIMESHEEPS_WATCHDOG=1`) rather than silently installing one.
    if cfg!(debug_assertions) && std::env::var_os("TIMESHEEPS_WATCHDOG").is_none() {
        log::info!("watchdog: skipped for a debug build (set TIMESHEEPS_WATCHDOG=1 to install)");
        return Ok(());
    }

    let exe = std::env::current_exe().map_err(|e| format!("cannot locate own executable: {e}"))?;
    let xml = task_xml(&exe);

    // `schtasks /XML` wants UTF-16 with a BOM; UTF-8 is rejected outright.
    let path = std::env::temp_dir().join("timesheeps-watchdog.xml");
    let mut bytes = vec![0xFF, 0xFE];
    for unit in xml.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    std::fs::write(&path, &bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))?;

    let path_str = path.to_string_lossy().to_string();
    let result = schtasks(&["/Create", "/TN", TASK_NAME, "/XML", &path_str, "/F"]);
    let _ = std::fs::remove_file(&path);

    match result {
        Ok(out) if out.status.success() => {
            log::info!(
                "watchdog: scheduled task installed (every {REPEAT_MINUTES} min, exe {})",
                exe.display()
            );
            Ok(())
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            Err(format!(
                "could not create the scheduled task: {}{}",
                stderr.trim(),
                if stderr.trim().is_empty() {
                    stdout.trim().to_string()
                } else {
                    format!(" ({})", stdout.trim())
                }
            ))
        }
        Err(e) => Err(e),
    }
}

/// The task definition.
///
/// Two triggers, because they cover different failures:
///
/// * `LogonTrigger` — starts the app as soon as the user is back, with a short
///   delay so it does not race Explorer and the network.
/// * `CalendarTrigger` repeating every [`REPEAT_MINUTES`] — the actual fix. This
///   is what recovers from a crash, a silent early exit, or the user closing the
///   app, without waiting for a reboot.
///
/// `ExecutionTimeLimit` **must** be `PT0S`: the launched process *is* the task,
/// so any finite limit would kill a running Timesheeps out from under the user.
#[cfg(windows)]
fn task_xml(exe: &std::path::Path) -> String {
    let user = current_user();
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Restarts Timesheeps if it is not running. Harmless while it is, because Timesheeps is single-instance.</Description>
    <URI>\{task_name}</URI>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
      <Delay>PT30S</Delay>
    </LogonTrigger>
    <CalendarTrigger>
      <StartBoundary>2020-01-01T00:00:00</StartBoundary>
      <Enabled>true</Enabled>
      <Repetition>
        <Interval>PT{minutes}M</Interval>
        <Duration>P1D</Duration>
        <StopAtDurationEnd>false</StopAtDurationEnd>
      </Repetition>
      <ScheduleByDay>
        <DaysInterval>1</DaysInterval>
      </ScheduleByDay>
    </CalendarTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{command}</Command>
      <Arguments>--hidden</Arguments>
    </Exec>
  </Actions>
</Task>"#,
        task_name = xml_escape(TASK_NAME),
        user = xml_escape(&user),
        minutes = REPEAT_MINUTES,
        command = xml_escape(&exe.display().to_string()),
    )
}

/// `DOMAIN\user`, which is what `schtasks` expects for a logon trigger.
#[cfg(windows)]
fn current_user() -> String {
    let user = std::env::var("USERNAME").unwrap_or_default();
    match std::env::var("USERDOMAIN") {
        Ok(domain) if !domain.is_empty() && !user.is_empty() => format!("{domain}\\{user}"),
        _ => user,
    }
}

#[cfg(windows)]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn xml_escape_covers_the_four_specials() {
        assert_eq!(xml_escape("a&b<c>d\"e"), "a&amp;b&lt;c&gt;d&quot;e");
        assert_eq!(xml_escape("plain"), "plain");
    }

    #[test]
    fn task_xml_repeats_forever_and_never_times_out() {
        let xml = task_xml(std::path::Path::new(r"C:\Apps\timesheeps.exe"));

        // The repeating trigger is the whole point: a logon-only trigger cannot
        // recover from a launch that died.
        assert!(xml.contains("<Interval>PT15M</Interval>"));
        assert!(xml.contains("<LogonTrigger>"));

        // A finite limit would kill a running Timesheeps out from under the user,
        // because the launched process *is* the task.
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));

        // Autostart launches must not steal focus.
        assert!(xml.contains("<Arguments>--hidden</Arguments>"));
        assert!(xml.contains(r"<Command>C:\Apps\timesheeps.exe</Command>"));
    }

    #[test]
    fn task_xml_escapes_the_executable_path() {
        let xml = task_xml(std::path::Path::new(r"C:\A & B\<x>.exe"));
        assert!(xml.contains(r"<Command>C:\A &amp; B\&lt;x&gt;.exe</Command>"));
        // A raw '&' would make the document invalid and `schtasks /XML` reject it.
        assert!(!xml.contains(r"C:\A & B"));
    }
}
