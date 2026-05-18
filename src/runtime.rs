use std::convert::TryFrom;
use std::ptr;

use framework_lib::chromium_ec::commands::{EcFeatureCode, EcRequestGetFeatures};
use framework_lib::chromium_ec::{CrosEc, CrosEcDriverType, EcRequestRaw};

use crate::results::ec_handle_result;
use crate::{FrameworkEcDriver, FrameworkEcHandle, FrameworkStatus, FrameworkStatusCode};

#[cfg(target_os = "linux")]
const CROS_EC_DEV_PATH: &str = "/dev/cros_ec";

fn default_ec_handle() -> Option<FrameworkEcHandle> {
    #[cfg(windows)]
    if let Some(ec) = CrosEc::with(CrosEcDriverType::Windows) {
        return Some(FrameworkEcHandle {
            ec,
            driver: FrameworkEcDriver::Windows,
        });
    }

    #[cfg(target_os = "linux")]
    if std::path::Path::new(CROS_EC_DEV_PATH).exists() {
        if let Some(ec) = CrosEc::with(CrosEcDriverType::CrosEc) {
            return Some(FrameworkEcHandle {
                ec,
                driver: FrameworkEcDriver::CrosEc,
            });
        }
    }

    #[cfg(all(not(windows), target_arch = "x86_64"))]
    if let Some(ec) = CrosEc::with(CrosEcDriverType::Portio) {
        return Some(FrameworkEcHandle {
            ec,
            driver: FrameworkEcDriver::Portio,
        });
    }

    None
}

pub(crate) fn driver_is_supported(driver: FrameworkEcDriver) -> bool {
    let Ok(driver) = CrosEcDriverType::try_from(driver) else {
        return false;
    };

    CrosEc::with(driver).is_some()
}

pub(crate) fn open_default_ec() -> crate::FrameworkEcHandleResult {
    let Some(handle) = default_ec_handle() else {
        return ec_handle_result(
            FrameworkStatus::with(FrameworkStatusCode::NoDriverAvailable, 0),
            ptr::null_mut(),
        );
    };

    if let Err(error) = handle.ec.check_mem_magic() {
        return ec_handle_result(crate::status::status_from_error(error), ptr::null_mut());
    }

    ec_handle_result(FrameworkStatus::success(), Box::into_raw(Box::new(handle)))
}

pub(crate) fn open_with_driver_ec(driver: FrameworkEcDriver) -> crate::FrameworkEcHandleResult {
    let Ok(driver_type) = CrosEcDriverType::try_from(driver) else {
        return ec_handle_result(
            FrameworkStatus::with(FrameworkStatusCode::UnsupportedDriver, 0),
            ptr::null_mut(),
        );
    };

    let Some(ec) = CrosEc::with(driver_type) else {
        return ec_handle_result(
            FrameworkStatus::with(FrameworkStatusCode::UnsupportedDriver, 0),
            ptr::null_mut(),
        );
    };

    if let Err(error) = ec.check_mem_magic() {
        return ec_handle_result(crate::status::status_from_error(error), ptr::null_mut());
    }

    ec_handle_result(
        FrameworkStatus::success(),
        Box::into_raw(Box::new(FrameworkEcHandle { ec, driver })),
    )
}

fn read_feature_flags(ec: &CrosEc) -> Result<[u32; 2], FrameworkStatus> {
    EcRequestGetFeatures {}
        .send_command(ec)
        .map(|response| response.flags)
        .map_err(crate::status::status_from_error)
}

pub(crate) fn feature_enabled(
    ec: &CrosEc,
    feature: EcFeatureCode,
) -> Result<bool, FrameworkStatus> {
    let flags = read_feature_flags(ec)?;
    let index = feature as usize;
    let word = index / 32;
    let bit = index % 32;
    Ok((flags[word] & (1 << bit)) != 0)
}

pub(crate) fn require_handle<'a>(
    handle: *const FrameworkEcHandle,
) -> Result<&'a FrameworkEcHandle, FrameworkStatus> {
    if handle.is_null() {
        return Err(FrameworkStatus::with(FrameworkStatusCode::NullPointer, 0));
    }

    // SAFETY: the caller guarantees the handle pointer came from framework_ec_open_*.
    Ok(unsafe { &*handle })
}

pub(crate) fn parse_optional_fan_index(fan_index: i32) -> Result<Option<u32>, FrameworkStatus> {
    if fan_index == -1 {
        return Ok(None);
    }

    let fan_index = u32::try_from(fan_index)
        .map_err(|_| FrameworkStatus::with(FrameworkStatusCode::InvalidArgument, fan_index))?;
    Ok(Some(fan_index))
}

pub(crate) fn parse_optional_fan_index_u8(fan_index: i32) -> Result<Option<u8>, FrameworkStatus> {
    if fan_index == -1 {
        return Ok(None);
    }

    let fan_index = u8::try_from(fan_index)
        .map_err(|_| FrameworkStatus::with(FrameworkStatusCode::InvalidArgument, fan_index))?;
    Ok(Some(fan_index))
}
