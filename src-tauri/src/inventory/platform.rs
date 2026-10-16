use serde::Serialize;
use std::path::Path;

#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{drives, empty_recycle_bin, recycle, recycle_bytes};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Drive {
    pub path: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

#[cfg(windows)]
pub fn drives() -> Vec<Drive> {
    use windows::{
        core::PCWSTR,
        Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives},
    };
    let mut result = vec![];
    unsafe {
        let mask = GetLogicalDrives();
        for i in 0..26 {
            if mask & (1 << i) == 0 {
                continue;
            }
            let path = format!("{}:\\", (b'A' + i) as char);
            let wide: Vec<_> = path.encode_utf16().chain(Some(0)).collect();
            if GetDriveTypeW(PCWSTR(wide.as_ptr())) != 3 {
                continue;
            }
            let (mut total, mut free) = (0, 0);
            if GetDiskFreeSpaceExW(
                PCWSTR(wide.as_ptr()),
                None,
                Some(&mut total),
                Some(&mut free),
            )
            .is_ok()
            {
                result.push(Drive {
                    path,
                    total_bytes: total,
                    free_bytes: free,
                });
            }
        }
    }
    result
}
#[cfg(all(not(windows), not(target_os = "macos")))]
pub fn drives() -> Vec<Drive> {
    vec![]
}

#[cfg(windows)]
pub fn recycle_bytes() -> u64 {
    use windows::{
        core::PCWSTR,
        Win32::UI::Shell::{SHQueryRecycleBinW, SHQUERYRBINFO},
    };
    let mut info = SHQUERYRBINFO {
        cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        if SHQueryRecycleBinW(PCWSTR::null(), &mut info).is_ok() {
            info.i64Size.max(0) as u64
        } else {
            0
        }
    }
}
#[cfg(all(not(windows), not(target_os = "macos")))]
pub fn recycle_bytes() -> u64 {
    0
}

#[cfg(windows)]
pub fn recycle(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        core::PCWSTR,
        Win32::{System::Com::*, UI::Shell::*},
    };
    // Do not silently convert a recycle request into a permanent delete.
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|e| e.to_string())?;
        struct ComGuard;
        impl Drop for ComGuard {
            fn drop(&mut self) {
                unsafe {
                    CoUninitialize();
                }
            }
        }
        let _guard = ComGuard;
        let op: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| e.to_string())?;
        let flags = FOFX_RECYCLEONDELETE
            | FOFX_ADDUNDORECORD
            | FOFX_EARLYFAILURE
            | FOF_NOERRORUI
            | FOF_SILENT
            | FOF_NOCONFIRMATION
            | FOF_WANTNUKEWARNING
            | FOF_NO_CONNECTED_ELEMENTS;
        op.SetOperationFlags(flags).map_err(|e| e.to_string())?;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let item: IShellItem =
            SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None).map_err(|e| e.to_string())?;
        let succeeded = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let sink: IFileOperationProgressSink = super::recycle_sink::RecycleOnly {
            succeeded: succeeded.clone(),
        }
        .into();
        op.DeleteItem(&item, &sink).map_err(|e| e.to_string())?;
        op.PerformOperations().map_err(|e| e.to_string())?;
        if op
            .GetAnyOperationsAborted()
            .map_err(|e| e.to_string())?
            .as_bool()
            || path.exists()
            || !succeeded.load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err("回收站操作取消或未确认完成；请检查原路径和回收站".into());
        }
    }
    Ok(())
}
#[cfg(all(not(windows), not(target_os = "macos")))]
pub fn recycle(_: &Path) -> Result<(), String> {
    Err("回收站清理仅支持 Windows".into())
}

#[cfg(windows)]
pub fn empty_recycle_bin() -> Result<(), String> {
    use windows::{core::PCWSTR, Win32::UI::Shell::*};
    // Keep the native confirmation as a second explicit checkpoint.
    unsafe {
        SHEmptyRecycleBinW(None, PCWSTR::null(), SHERB_NOPROGRESSUI | SHERB_NOSOUND)
            .map_err(|e| e.to_string())
    }
}
#[cfg(all(not(windows), not(target_os = "macos")))]
pub fn empty_recycle_bin() -> Result<(), String> {
    Err("仅支持 Windows".into())
}

pub fn reveal(path: &Path) -> Result<(), String> {
    super::snapshot::safe_path(path)?;
    #[cfg(windows)]
    {
        std::process::Command::new("explorer.exe")
            .arg("/select,")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        macos::reveal(path)
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        Err("仅支持 Windows".into())
    }
}
pub fn open_system(target: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        let argument = match target {
            "recycle" => "shell:RecycleBinFolder",
            "storage" => "ms-settings:storagesense",
            _ => return Err("不支持的入口".into()),
        };
        std::process::Command::new("explorer.exe")
            .arg(argument)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        macos::open_system(target)
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = target;
        Err("仅支持 Windows".into())
    }
}
