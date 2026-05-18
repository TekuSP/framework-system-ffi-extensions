use std::slice;

use framework_lib::chromium_ec::GpuCfgDescriptor;

use crate::{
    FrameworkByteBuffer, FrameworkEcGpuDescriptorHeaderResult, FrameworkEcGpuDescriptorReadResult,
    FrameworkEcGpuDescriptorValidationResult, FrameworkEcHandle, FrameworkGpuDescriptorHeader,
    FrameworkStatus, FrameworkStatusCode,
};

pub(crate) fn default_header() -> FrameworkGpuDescriptorHeader {
    FrameworkGpuDescriptorHeader {
        magic: [0; 4],
        length: 0,
        desc_ver_major: 0,
        desc_ver_minor: 0,
        hardware_version: 0,
        hardware_revision: 0,
        serial: [0; 20],
        descriptor_length: 0,
        descriptor_crc32: 0,
        crc32: 0,
    }
}

pub(crate) fn default_header_result() -> FrameworkEcGpuDescriptorHeaderResult {
    FrameworkEcGpuDescriptorHeaderResult {
        status: FrameworkStatus::success(),
        header: default_header(),
    }
}

pub(crate) fn default_read_result() -> FrameworkEcGpuDescriptorReadResult {
    FrameworkEcGpuDescriptorReadResult {
        status: FrameworkStatus::success(),
        descriptor: FrameworkByteBuffer::default(),
    }
}

pub(crate) fn default_validation_result() -> FrameworkEcGpuDescriptorValidationResult {
    FrameworkEcGpuDescriptorValidationResult {
        status: FrameworkStatus::success(),
        is_match: 0,
        reserved: [0; 3],
    }
}

pub(crate) fn from_upstream_header(header: GpuCfgDescriptor) -> FrameworkGpuDescriptorHeader {
    FrameworkGpuDescriptorHeader {
        magic: header.magic,
        length: header.length,
        desc_ver_major: header.desc_ver_major,
        desc_ver_minor: header.desc_ver_minor,
        hardware_version: header.hardware_version,
        hardware_revision: header.hardware_revision,
        serial: header.serial,
        descriptor_length: header.descriptor_length,
        descriptor_crc32: header.descriptor_crc32,
        crc32: header.crc32,
    }
}

fn ensure_descriptor_present(handle: &FrameworkEcHandle) -> Result<(), FrameworkStatus> {
    let bay = crate::inventory::expansion_bay_status(&handle.ec)?;
    if bay.present == 0 {
        return Err(FrameworkStatus::with(
            FrameworkStatusCode::DataUnavailable,
            0,
        ));
    }

    Ok(())
}

pub(crate) fn read_header(
    handle: &FrameworkEcHandle,
) -> Result<FrameworkGpuDescriptorHeader, FrameworkStatus> {
    ensure_descriptor_present(handle)?;

    handle
        .ec
        .read_gpu_desc_header()
        .map(from_upstream_header)
        .map_err(crate::status_from_error)
}

pub(crate) fn read_raw_descriptor(
    handle: &FrameworkEcHandle,
) -> Result<FrameworkByteBuffer, FrameworkStatus> {
    ensure_descriptor_present(handle)?;

    let descriptor = handle
        .ec
        .read_gpu_descriptor()
        .map_err(crate::status_from_error)?;
    if descriptor.is_empty() {
        return Err(FrameworkStatus::with(
            FrameworkStatusCode::DataUnavailable,
            0,
        ));
    }

    Ok(FrameworkByteBuffer::from_vec(descriptor))
}

pub(crate) unsafe fn validate_expected_bytes<'a>(
    expected_descriptor_ptr: *const u8,
    expected_descriptor_length: u32,
) -> Result<&'a [u8], FrameworkStatus> {
    let length = expected_descriptor_length as usize;
    if length == 0 {
        return Ok(&[]);
    }

    if expected_descriptor_ptr.is_null() {
        return Err(FrameworkStatus::with(FrameworkStatusCode::NullPointer, 0));
    }

    Ok(slice::from_raw_parts(expected_descriptor_ptr, length))
}

pub(crate) fn validate(
    handle: &FrameworkEcHandle,
    expected_descriptor: &[u8],
) -> Result<bool, FrameworkStatus> {
    ensure_descriptor_present(handle)?;

    handle
        .ec
        .validate_gpu_descriptor(expected_descriptor)
        .map_err(crate::status_from_error)
}
