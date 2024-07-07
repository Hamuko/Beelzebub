use futures::Stream;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use log::{debug, warn};
use serde::Deserialize;
use windows::{
    core::{HSTRING, PCWSTR},
    Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW},
};
use wmi::{COMLibrary, FilterValue, WMIConnection, WMIError};
pub type ProcessStartResult = Result<ProcessStartEvent, WMIError>;
pub type ProcessEndResult = Result<ProcessEndEvent, WMIError>;

#[derive(Deserialize, Debug)]
#[serde(rename = "__InstanceCreationEvent")]
#[serde(rename_all = "PascalCase")]
pub struct ProcessStartEvent {
    pub target_instance: Process,
}
#[derive(Deserialize, Debug)]
#[serde(rename = "__InstanceDeletionEvent")]
#[serde(rename_all = "PascalCase")]
pub struct ProcessEndEvent {
    pub target_instance: Process,
}

#[derive(Deserialize, Debug)]
#[serde(rename = "Win32_Process")]
#[serde(rename_all = "PascalCase")]
pub struct Process {
    pub process_id: u32,
    pub name: String,
    pub executable_path: Option<String>,
    parent_process_id: u32,
}

impl Process {
    /// Fetch the executable product name for prettier reporting.
    pub fn get_display_name(&self) -> Option<String> {
        let executable_path = match &self.executable_path {
            Some(path) => Path::new(path),
            None => return None,
        };
        let filename = &HSTRING::from(executable_path.as_os_str());

        let version_info_size = unsafe { GetFileVersionInfoSizeW(filename, None) };
        if version_info_size == 0 {
            warn!(
                "Could not retrieve product name: \
                could not get version info size"
            );
            return None;
        }

        let mut version_info_buffer = Vec::<u8>::with_capacity(version_info_size as usize);
        unsafe {
            let version_info_success = GetFileVersionInfoW(
                filename,
                0,
                version_info_size,
                version_info_buffer.as_mut_ptr() as *mut std::ffi::c_void,
            );
            if version_info_success.is_err() {
                warn!(
                    "Could not retrieve product name for {}: \
                    could not get version info",
                    executable_path.display()
                );
                return None;
            };
        }

        let mut lang_code_pages_ptr = std::ptr::null_mut();
        let mut lang_code_pages_length = 0;
        unsafe {
            let query_success = VerQueryValueW(
                version_info_buffer.as_mut_ptr() as *mut std::ffi::c_void,
                windows::core::w!("\\VarFileInfo\\Translation"),
                &mut lang_code_pages_ptr,
                &mut lang_code_pages_length,
            )
            .as_bool();
            if !query_success {
                warn!(
                    "Could not retrieve product name for {}: \
                    couldn't query translation info",
                    executable_path.display()
                );
                return None;
            }
        }
        if lang_code_pages_length == 0 {
            warn!(
                "Could not retrieve product name for {}: no translation info",
                executable_path.display()
            );
            return None;
        }
        let lang_code_pages = unsafe {
            std::slice::from_raw_parts::<(u16, u16)>(
                lang_code_pages_ptr.cast(),
                lang_code_pages_length as usize,
            )
        };

        for lang_code_page in lang_code_pages {
            debug!("{:?}", lang_code_page);
            let sub_block = format!(
                "\\StringFileInfo\\{:04x}{:04x}\\ProductName\0",
                lang_code_page.0, lang_code_page.1,
            )
            .encode_utf16()
            .collect::<Vec<u16>>();
            let mut product_name_ptr = std::ptr::null_mut();
            let mut product_name_length = 0;
            unsafe {
                let query_success = VerQueryValueW(
                    version_info_buffer.as_mut_ptr() as *mut std::ffi::c_void,
                    PCWSTR::from_raw(sub_block.as_ptr()),
                    &mut product_name_ptr,
                    &mut product_name_length,
                )
                .as_bool();
                if !query_success {
                    debug!(
                        "Could not retrieve product name for language {:04x}{:04x}: \
                        couldn't query product name",
                        lang_code_page.0, lang_code_page.1
                    );
                    continue;
                }
            }
            if product_name_length == 0 {
                debug!(
                    "Could not retrieve product name for language {:04x}{:04x}: \
                    no product name",
                    lang_code_page.0, lang_code_page.1
                );
                continue;
            }
            let product_name = unsafe {
                std::slice::from_raw_parts(
                    product_name_ptr.cast(),
                    product_name_length as usize - 1,
                )
            };
            let product_name = String::from_utf16_lossy(product_name);
            return Some(product_name);
        }

        warn!(
            "Could not determine product name for {}",
            executable_path.display()
        );
        return None;
    }
}

pub fn create_streams() -> Result<
    (
        impl Stream<Item = ProcessStartResult>,
        impl Stream<Item = ProcessEndResult>,
    ),
    WMIError,
> {
    let com_con = COMLibrary::new()?;
    let wmi = WMIConnection::new(com_con)?;

    let mut filters = HashMap::<String, FilterValue>::new();
    filters.insert(
        "TargetInstance".to_owned(),
        FilterValue::is_a::<Process>().unwrap(),
    );

    let stream_start = wmi
        .async_filtered_notification::<ProcessStartEvent>(&filters, Some(Duration::from_secs(1)))?;
    let stream_end =
        wmi.async_filtered_notification::<ProcessEndEvent>(&filters, Some(Duration::from_secs(1)))?;
    return Ok((stream_start, stream_end));
}
