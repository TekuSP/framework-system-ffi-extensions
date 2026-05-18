use std::collections::VecDeque;
use std::convert::TryFrom;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

use framework_lib::chromium_ec::{EcError, EcResponseStatus};

use crate::{
    FrameworkEcResponseDetail, FrameworkStatus, FrameworkStatusCode,
    FrameworkStatusDeviceErrorRecord, FrameworkStatusEcResponseRecord,
    FrameworkStatusInvalidFanIndexRecord, FrameworkStatusNoPayload, FrameworkStatusPayload,
    FrameworkStatusUnknownEcResponseCodeRecord,
};

const STORED_DEVICE_ERROR_LIMIT: usize = 64;
static NEXT_DEVICE_ERROR_ID: AtomicI32 = AtomicI32::new(1);
static DEVICE_ERROR_MESSAGES: OnceLock<Mutex<VecDeque<(i32, String)>>> = OnceLock::new();

fn device_error_messages() -> &'static Mutex<VecDeque<(i32, String)>> {
    DEVICE_ERROR_MESSAGES.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn store_device_error_message(message: String) -> i32 {
    let id = NEXT_DEVICE_ERROR_ID.fetch_add(1, Ordering::Relaxed);
    let mut messages = device_error_messages()
        .lock()
        .expect("device error message lock poisoned");
    messages.push_back((id, message));
    while messages.len() > STORED_DEVICE_ERROR_LIMIT {
        messages.pop_front();
    }
    id
}

pub(crate) fn get_device_error_message(detail: i32) -> Option<String> {
    if detail <= 0 {
        return None;
    }

    let messages = device_error_messages().lock().ok()?;
    messages
        .iter()
        .find(|(id, _)| *id == detail)
        .map(|(_, message)| message.clone())
}

fn ec_response_detail_from_raw(detail: i32) -> FrameworkEcResponseDetail {
    match detail {
        0 => FrameworkEcResponseDetail::Success,
        1 => FrameworkEcResponseDetail::InvalidCommand,
        2 => FrameworkEcResponseDetail::Error,
        3 => FrameworkEcResponseDetail::InvalidParameter,
        4 => FrameworkEcResponseDetail::AccessDenied,
        5 => FrameworkEcResponseDetail::InvalidResponse,
        6 => FrameworkEcResponseDetail::InvalidVersion,
        7 => FrameworkEcResponseDetail::InvalidChecksum,
        8 => FrameworkEcResponseDetail::InProgress,
        9 => FrameworkEcResponseDetail::Unavailable,
        10 => FrameworkEcResponseDetail::Timeout,
        11 => FrameworkEcResponseDetail::Overflow,
        12 => FrameworkEcResponseDetail::InvalidHeader,
        13 => FrameworkEcResponseDetail::RequestTruncated,
        14 => FrameworkEcResponseDetail::ResponseTooBig,
        15 => FrameworkEcResponseDetail::BusError,
        16 => FrameworkEcResponseDetail::Busy,
        _ => FrameworkEcResponseDetail::Unknown,
    }
}

fn ec_response_detail_name(detail: FrameworkEcResponseDetail) -> &'static str {
    match detail {
        FrameworkEcResponseDetail::Unknown => "Unknown",
        FrameworkEcResponseDetail::Success => "Success",
        FrameworkEcResponseDetail::InvalidCommand => "InvalidCommand",
        FrameworkEcResponseDetail::Error => "Error",
        FrameworkEcResponseDetail::InvalidParameter => "InvalidParameter",
        FrameworkEcResponseDetail::AccessDenied => "AccessDenied",
        FrameworkEcResponseDetail::InvalidResponse => "InvalidResponse",
        FrameworkEcResponseDetail::InvalidVersion => "InvalidVersion",
        FrameworkEcResponseDetail::InvalidChecksum => "InvalidChecksum",
        FrameworkEcResponseDetail::InProgress => "InProgress",
        FrameworkEcResponseDetail::Unavailable => "Unavailable",
        FrameworkEcResponseDetail::Timeout => "Timeout",
        FrameworkEcResponseDetail::Overflow => "Overflow",
        FrameworkEcResponseDetail::InvalidHeader => "InvalidHeader",
        FrameworkEcResponseDetail::RequestTruncated => "RequestTruncated",
        FrameworkEcResponseDetail::ResponseTooBig => "ResponseTooBig",
        FrameworkEcResponseDetail::BusError => "BusError",
        FrameworkEcResponseDetail::Busy => "Busy",
    }
}

impl FrameworkStatus {
    pub(crate) fn success() -> Self {
        Self::no_payload(FrameworkStatusCode::Success)
    }

    pub(crate) fn with(code: FrameworkStatusCode, detail: i32) -> Self {
        match code {
            FrameworkStatusCode::Success => Self::success(),
            FrameworkStatusCode::NullPointer => Self::no_payload(FrameworkStatusCode::NullPointer),
            FrameworkStatusCode::InvalidArgument => Self::invalid_fan_index(detail),
            FrameworkStatusCode::NoDriverAvailable => {
                Self::no_payload(FrameworkStatusCode::NoDriverAvailable)
            }
            FrameworkStatusCode::UnsupportedDriver => {
                Self::no_payload(FrameworkStatusCode::UnsupportedDriver)
            }
            FrameworkStatusCode::DeviceError => Self::device_error(detail),
            FrameworkStatusCode::EcResponse => {
                Self::ec_response(ec_response_detail_from_raw(detail))
            }
            FrameworkStatusCode::UnknownResponseCode => Self::unknown_response_code(detail),
            FrameworkStatusCode::DataUnavailable => {
                Self::no_payload(FrameworkStatusCode::DataUnavailable)
            }
        }
    }

    fn no_payload(code: FrameworkStatusCode) -> Self {
        Self {
            code,
            payload: FrameworkStatusPayload {
                none: FrameworkStatusNoPayload { reserved: 0 },
            },
        }
    }

    fn invalid_fan_index(fan_index: i32) -> Self {
        Self {
            code: FrameworkStatusCode::InvalidArgument,
            payload: FrameworkStatusPayload {
                invalid_fan_index: FrameworkStatusInvalidFanIndexRecord { fan_index },
            },
        }
    }

    fn ec_response(response: FrameworkEcResponseDetail) -> Self {
        Self {
            code: FrameworkStatusCode::EcResponse,
            payload: FrameworkStatusPayload {
                ec_response: FrameworkStatusEcResponseRecord { response },
            },
        }
    }

    fn unknown_response_code(response_code: i32) -> Self {
        Self {
            code: FrameworkStatusCode::UnknownResponseCode,
            payload: FrameworkStatusPayload {
                unknown_ec_response_code: FrameworkStatusUnknownEcResponseCodeRecord {
                    response_code,
                },
            },
        }
    }

    fn device_error(message_token: i32) -> Self {
        Self {
            code: FrameworkStatusCode::DeviceError,
            payload: FrameworkStatusPayload {
                device_error: FrameworkStatusDeviceErrorRecord { message_token },
            },
        }
    }

    pub(crate) fn invalid_fan_index_value(&self) -> Option<i32> {
        if self.code != FrameworkStatusCode::InvalidArgument {
            return None;
        }

        Some(unsafe { self.payload.invalid_fan_index.fan_index })
    }

    pub(crate) fn ec_response_detail(&self) -> Option<FrameworkEcResponseDetail> {
        if self.code != FrameworkStatusCode::EcResponse {
            return None;
        }

        Some(unsafe { self.payload.ec_response.response })
    }

    pub(crate) fn unknown_response_code_value(&self) -> Option<i32> {
        if self.code != FrameworkStatusCode::UnknownResponseCode {
            return None;
        }

        Some(unsafe { self.payload.unknown_ec_response_code.response_code })
    }

    pub(crate) fn device_error_message_token(&self) -> Option<i32> {
        if self.code != FrameworkStatusCode::DeviceError {
            return None;
        }

        Some(unsafe { self.payload.device_error.message_token })
    }
}

impl From<EcResponseStatus> for FrameworkEcResponseDetail {
    fn from(value: EcResponseStatus) -> Self {
        match value {
            EcResponseStatus::Success => FrameworkEcResponseDetail::Success,
            EcResponseStatus::InvalidCommand => FrameworkEcResponseDetail::InvalidCommand,
            EcResponseStatus::Error => FrameworkEcResponseDetail::Error,
            EcResponseStatus::InvalidParameter => FrameworkEcResponseDetail::InvalidParameter,
            EcResponseStatus::AccessDenied => FrameworkEcResponseDetail::AccessDenied,
            EcResponseStatus::InvalidResponse => FrameworkEcResponseDetail::InvalidResponse,
            EcResponseStatus::InvalidVersion => FrameworkEcResponseDetail::InvalidVersion,
            EcResponseStatus::InvalidChecksum => FrameworkEcResponseDetail::InvalidChecksum,
            EcResponseStatus::InProgress => FrameworkEcResponseDetail::InProgress,
            EcResponseStatus::Unavailable => FrameworkEcResponseDetail::Unavailable,
            EcResponseStatus::Timeout => FrameworkEcResponseDetail::Timeout,
            EcResponseStatus::Overflow => FrameworkEcResponseDetail::Overflow,
            EcResponseStatus::InvalidHeader => FrameworkEcResponseDetail::InvalidHeader,
            EcResponseStatus::RequestTruncated => FrameworkEcResponseDetail::RequestTruncated,
            EcResponseStatus::ResponseTooBig => FrameworkEcResponseDetail::ResponseTooBig,
            EcResponseStatus::BusError => FrameworkEcResponseDetail::BusError,
            EcResponseStatus::Busy => FrameworkEcResponseDetail::Busy,
        }
    }
}

pub(crate) fn status_description(status: FrameworkStatus) -> String {
    match status.code {
        FrameworkStatusCode::Success => "Success".to_string(),
        FrameworkStatusCode::NullPointer => "Null pointer".to_string(),
        FrameworkStatusCode::InvalidArgument => {
            format!(
                "Invalid fan index: {}",
                status.invalid_fan_index_value().unwrap_or_default()
            )
        }
        FrameworkStatusCode::NoDriverAvailable => "No EC driver available".to_string(),
        FrameworkStatusCode::UnsupportedDriver => {
            "Requested EC driver is not supported on this system".to_string()
        }
        FrameworkStatusCode::DeviceError => {
            if let Some(message) = status
                .device_error_message_token()
                .and_then(get_device_error_message)
            {
                format!("Device error: {}", message)
            } else {
                "Device error".to_string()
            }
        }
        FrameworkStatusCode::EcResponse => {
            let detail = status
                .ec_response_detail()
                .unwrap_or(FrameworkEcResponseDetail::Unknown);
            format!(
                "EC response: {} ({})",
                ec_response_detail_name(detail),
                detail as i32
            )
        }
        FrameworkStatusCode::UnknownResponseCode => {
            format!(
                "Unknown EC response code: {}",
                status.unknown_response_code_value().unwrap_or_default()
            )
        }
        FrameworkStatusCode::DataUnavailable => "Data unavailable".to_string(),
    }
}

pub(crate) fn status_from_error(error: EcError) -> FrameworkStatus {
    match error {
        EcError::Response(response) => {
            FrameworkStatus::with(FrameworkStatusCode::EcResponse, response as i32)
        }
        EcError::UnknownResponseCode(code) => FrameworkStatus::with(
            FrameworkStatusCode::UnknownResponseCode,
            i32::try_from(code).unwrap_or(i32::MAX),
        ),
        EcError::DeviceError(message) => {
            let detail = store_device_error_message(message);
            FrameworkStatus::with(FrameworkStatusCode::DeviceError, detail)
        }
    }
}
