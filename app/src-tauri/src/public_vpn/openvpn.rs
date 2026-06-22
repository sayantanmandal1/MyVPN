//! Launch and control the bundled OpenVPN client for the public VPN mode.
//!
//! We talk to OpenVPN over its localhost **management interface** so we can both
//! observe connection state (`>STATE:` notifications) and shut it down cleanly
//! (`signal SIGTERM`), which lets OpenVPN remove its own routes/DNS on exit
//! instead of leaving them behind after a hard process kill.

use std::net::TcpListener;
use std::path::{Path, PathBuf};

use anyhow::Context;
use std::process::{Child, Command, Stdio};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(windows)]
use win::ElevatedChild;

/// Locate the OpenVPN executable. Prefers the copy bundled inside the installed
/// app (no separate OpenVPN install is required); falls back to a system-wide
/// install if, for some reason, the bundle is missing.
pub fn find_openvpn(resource_dir: &Path) -> Option<PathBuf> {
    let mut candidates = vec![
        resource_dir.join("openvpn").join("openvpn.exe"),
        resource_dir.join("openvpn.exe"),
    ];
    // Some packaging layouts place resources next to the executable rather than
    // under the reported resource dir; check those too.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("openvpn").join("openvpn.exe"));
            candidates.push(dir.join("resources").join("openvpn").join("openvpn.exe"));
        }
    }
    // Last resort: a system-wide OpenVPN install.
    candidates.push(PathBuf::from(r"C:\Program Files\OpenVPN\bin\openvpn.exe"));
    candidates.push(PathBuf::from(r"C:\Program Files (x86)\OpenVPN\bin\openvpn.exe"));

    candidates.into_iter().find(|p| p.exists())
}

/// Reserve a free localhost TCP port for the management interface. The listener
/// is dropped immediately; OpenVPN then binds the port itself.
fn free_port() -> anyhow::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").context("reserve management port")?;
    let port = listener.local_addr()?.port();
    Ok(port)
}

/// A handle to the running OpenVPN process.
///
/// OpenVPN must create the tunnel adapter and install routes, which needs
/// administrator rights. The app ships a requireAdministrator manifest, so it is
/// already elevated and a normal child inherits that token (`Child`). If for
/// some reason the app is *not* elevated, OpenVPN is self-elevated via UAC and
/// tracked as `Elevated`. Control always flows over the localhost management
/// interface, which works across the elevation boundary.
pub enum ProcHandle {
    Child(Child),
    #[cfg(windows)]
    Elevated(ElevatedChild),
}

impl ProcHandle {
    /// Whether the process has already exited (non-blocking).
    pub fn has_exited(&mut self) -> bool {
        match self {
            ProcHandle::Child(c) => matches!(c.try_wait(), Ok(Some(_))),
            #[cfg(windows)]
            ProcHandle::Elevated(e) => e.has_exited(),
        }
    }

    /// Force-terminate the process (best effort; graceful SIGTERM is preferred).
    pub fn kill(&mut self) {
        match self {
            ProcHandle::Child(c) => {
                let _ = c.kill();
                let _ = c.wait();
            }
            #[cfg(windows)]
            ProcHandle::Elevated(e) => e.kill(),
        }
    }
}

/// A spawned OpenVPN process plus the management port it listens on.
pub struct OpenVpnProcess {
    pub proc: ProcHandle,
    pub mgmt_port: u16,
}

/// The OpenVPN argument list shared by every launch path.
///
/// Compatibility cipher flags are added so the older configs published by free
/// servers still negotiate with a modern OpenVPN build; `block-outside-dns` is
/// filtered out so we don't require the privileged WFP/service component; and on
/// Windows we force the **Wintun** data-plane driver (bundled as `wintun.dll`
/// next to `openvpn.exe`) so no separately-installed TAP adapter is needed.
/// A log file is written so connection failures can be explained to the user.
fn ovpn_args(config: &Path, mgmt_port: u16, log: &Path) -> Vec<String> {
    let mut a = vec![
        "--config".to_string(),
        config.to_string_lossy().into_owned(),
        "--management".to_string(),
        "127.0.0.1".to_string(),
        mgmt_port.to_string(),
        "--management-query-passwords".to_string(),
        "--data-ciphers".to_string(),
        "AES-256-GCM:AES-128-GCM:AES-256-CBC:AES-128-CBC:BF-CBC".to_string(),
        "--data-ciphers-fallback".to_string(),
        "AES-128-CBC".to_string(),
        "--connect-retry-max".to_string(),
        "3".to_string(),
        "--pull-filter".to_string(),
        "ignore".to_string(),
        "block-outside-dns".to_string(),
        "--verb".to_string(),
        "3".to_string(),
        "--log".to_string(),
        log.to_string_lossy().into_owned(),
    ];
    #[cfg(windows)]
    {
        // Use the bundled userspace Wintun adapter rather than the default
        // TAP-Windows driver, which would otherwise have to be pre-installed.
        a.push("--windows-driver".to_string());
        a.push("wintun".to_string());
    }
    a
}

/// Spawn OpenVPN against a config file with a localhost management interface.
pub fn spawn(openvpn: &Path, config: &Path, work_dir: &Path) -> anyhow::Result<OpenVpnProcess> {
    let mgmt_port = free_port()?;
    let log = work_dir.join("openvpn.log");
    // Start each session with a fresh log so `log_reason` reflects this attempt.
    let _ = std::fs::remove_file(&log);
    let args = ovpn_args(config, mgmt_port, &log);
    let proc = spawn_inner(openvpn, &args, work_dir)?;
    Ok(OpenVpnProcess { proc, mgmt_port })
}

#[cfg(not(windows))]
fn spawn_inner(openvpn: &Path, args: &[String], work_dir: &Path) -> anyhow::Result<ProcHandle> {
    Ok(ProcHandle::Child(spawn_plain(openvpn, args, work_dir)?))
}

#[cfg(windows)]
fn spawn_inner(openvpn: &Path, args: &[String], work_dir: &Path) -> anyhow::Result<ProcHandle> {
    // The app ships a requireAdministrator manifest, so normally we already hold
    // an elevated token and a plain child inherits it — no prompt, no console
    // window. Only if we somehow aren't elevated do we self-elevate OpenVPN via
    // UAC so the feature still works.
    if win::is_elevated() {
        Ok(ProcHandle::Child(spawn_plain(openvpn, args, work_dir)?))
    } else {
        win::spawn_elevated(openvpn, args, work_dir).map(ProcHandle::Elevated)
    }
}

/// Launch OpenVPN as an ordinary child (inheriting our token). On Windows the
/// console window is suppressed.
fn spawn_plain(openvpn: &Path, args: &[String], work_dir: &Path) -> anyhow::Result<Child> {
    let mut cmd = Command::new(openvpn);
    cmd.args(args)
        .current_dir(work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.spawn().context("launch OpenVPN")
}

/// Distil a concise failure reason from OpenVPN's own log, so the UI can show
/// *why* a connection failed (e.g. needs administrator, adapter unavailable)
/// instead of a generic "Connection lost." Returns the last error-like line, or
/// the last line written, trimmed to a sensible length.
pub fn log_reason(work_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(work_dir.join("openvpn.log")).ok()?;
    const KEYS: [&str; 9] = [
        "error",
        "fatal",
        "cannot",
        "permission",
        "administrator",
        "all tap",
        "there are no",
        "failed",
        "exiting due to",
    ];
    let mut last_meaningful: Option<String> = None;
    let mut last_error: Option<String> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        last_meaningful = Some(line.to_string());
        let low = line.to_ascii_lowercase();
        if KEYS.iter().any(|k| low.contains(k)) {
            last_error = Some(line.to_string());
        }
    }
    let pick = last_error.or(last_meaningful)?;
    Some(truncate_chars(&pick, 240))
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Windows-only: launch OpenVPN elevated via UAC and manage the resulting
/// process handle. All of OpenVPN's privileged work (adapter creation, route
/// and DNS changes) needs administrator rights; the app itself stays
/// unelevated and drives OpenVPN over its localhost management socket.
#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::path::Path;

    use anyhow::{anyhow, Context};
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_CANCELLED, HANDLE, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::System::Threading::{TerminateProcess, WaitForSingleObject};

    // `windows-sys` 0.59 doesn't ship the Shell "execute" API, so bind the few
    // pieces needed to launch OpenVPN elevated (the `runas` verb triggers UAC).
    const SEE_MASK_NOCLOSEPROCESS: u32 = 0x0000_0040;
    const SW_HIDE: i32 = 0;

    #[repr(C)]
    #[allow(non_snake_case, dead_code, clippy::upper_case_acronyms)]
    struct SHELLEXECUTEINFOW {
        cbSize: u32,
        fMask: u32,
        hwnd: *mut c_void,
        lpVerb: *const u16,
        lpFile: *const u16,
        lpParameters: *const u16,
        lpDirectory: *const u16,
        nShow: i32,
        hInstApp: *mut c_void,
        lpIDList: *mut c_void,
        lpClass: *const u16,
        hkeyClass: *mut c_void,
        dwHotKey: u32,
        hIconOrMonitor: *mut c_void,
        hProcess: HANDLE,
    }

    #[allow(non_snake_case)]
    extern "system" {
        #[link_name = "ShellExecuteExW"]
        fn ShellExecuteExW(pExecInfo: *mut SHELLEXECUTEINFOW) -> i32;
    }

    /// Owns the handle of an elevated OpenVPN process. `isize` is `Send`/`Sync`,
    /// and the handle is closed on drop.
    pub struct ElevatedChild {
        handle: isize,
    }

    impl ElevatedChild {
        pub fn has_exited(&self) -> bool {
            if self.handle == 0 {
                return true;
            }
            // A zero timeout makes this a non-blocking poll.
            unsafe { WaitForSingleObject(self.handle as HANDLE, 0) == WAIT_OBJECT_0 }
        }

        pub fn kill(&self) {
            if self.handle == 0 {
                return;
            }
            unsafe {
                TerminateProcess(self.handle as HANDLE, 1);
                WaitForSingleObject(self.handle as HANDLE, 3000);
            }
        }
    }

    impl Drop for ElevatedChild {
        fn drop(&mut self) {
            if self.handle != 0 {
                unsafe {
                    CloseHandle(self.handle as HANDLE);
                }
            }
        }
    }

    /// Whether the current process holds an elevated (administrator) token.
    pub fn is_elevated() -> bool {
        use windows_sys::Win32::Security::{
            GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
        };
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
        unsafe {
            let mut token: HANDLE = std::ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return false;
            }
            let mut elevation = TOKEN_ELEVATION {
                TokenIsElevated: 0,
            };
            let mut ret_len = 0u32;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                &mut elevation as *mut _ as *mut c_void,
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut ret_len,
            );
            CloseHandle(token);
            ok != 0 && elevation.TokenIsElevated != 0
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Re-quote the argument vector into a single command line for ShellExecute,
    /// quoting any argument that contains whitespace. All arguments are
    /// app-controlled (constant flags plus app-generated file paths); none come
    /// from user input, so this cannot be used for argument injection.
    fn join_params(args: &[String]) -> String {
        args.iter()
            .map(|a| {
                if a.is_empty() || a.contains(' ') || a.contains('\t') {
                    format!("\"{a}\"")
                } else {
                    a.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn spawn_elevated(
        openvpn: &Path,
        args: &[String],
        work_dir: &Path,
    ) -> anyhow::Result<ElevatedChild> {
        let file = wide(openvpn.to_str().context("OpenVPN path is not valid Unicode")?);
        let params = wide(&join_params(args));
        let dir = wide(work_dir.to_str().unwrap_or("."));
        let verb = wide("runas");

        // SAFETY: all wide-string buffers outlive the call; the struct is fully
        // zero-initialised before the fields we use are set.
        let mut sei: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        sei.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        sei.fMask = SEE_MASK_NOCLOSEPROCESS;
        sei.lpVerb = verb.as_ptr();
        sei.lpFile = file.as_ptr();
        sei.lpParameters = params.as_ptr();
        sei.lpDirectory = dir.as_ptr();
        sei.nShow = SW_HIDE;

        let ok = unsafe { ShellExecuteExW(&mut sei) };
        if ok == 0 {
            let code = unsafe { GetLastError() };
            if code == ERROR_CANCELLED {
                return Err(anyhow!(
                    "Administrator approval is required to create the VPN tunnel. \
                     The request was cancelled — click Connect again and choose Yes."
                ));
            }
            return Err(anyhow!("Could not launch OpenVPN (Windows error {code})."));
        }
        if sei.hProcess.is_null() {
            return Err(anyhow!("OpenVPN failed to start."));
        }
        Ok(ElevatedChild {
            handle: sei.hProcess as isize,
        })
    }
}
