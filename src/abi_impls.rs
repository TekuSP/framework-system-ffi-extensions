use std::convert::TryFrom;

use framework_lib::chromium_ec::{CrosEcDriverType, EcCurrentImage};
use framework_lib::smbios::{Platform, PlatformFamily};

use crate::{
    FrameworkEcCurrentImage, FrameworkEcDriver, FrameworkPlatform, FrameworkPlatformFamily,
};

impl From<CrosEcDriverType> for FrameworkEcDriver {
    fn from(value: CrosEcDriverType) -> Self {
        match value {
            CrosEcDriverType::Portio => FrameworkEcDriver::Portio,
            CrosEcDriverType::CrosEc => FrameworkEcDriver::CrosEc,
            CrosEcDriverType::Windows => FrameworkEcDriver::Windows,
        }
    }
}

impl TryFrom<FrameworkEcDriver> for CrosEcDriverType {
    type Error = ();

    fn try_from(value: FrameworkEcDriver) -> Result<Self, Self::Error> {
        match value {
            FrameworkEcDriver::Unknown => Err(()),
            FrameworkEcDriver::Portio => Ok(CrosEcDriverType::Portio),
            FrameworkEcDriver::CrosEc => Ok(CrosEcDriverType::CrosEc),
            FrameworkEcDriver::Windows => Ok(CrosEcDriverType::Windows),
        }
    }
}

impl From<Platform> for FrameworkPlatform {
    fn from(value: Platform) -> Self {
        match value {
            Platform::Framework12IntelGen13 => FrameworkPlatform::Framework12IntelGen13,
            Platform::IntelGen11 => FrameworkPlatform::IntelGen11,
            Platform::IntelGen12 => FrameworkPlatform::IntelGen12,
            Platform::IntelGen13 => FrameworkPlatform::IntelGen13,
            Platform::IntelCoreUltra1 => FrameworkPlatform::IntelCoreUltra1,
            Platform::IntelCoreUltra3 => FrameworkPlatform::IntelCoreUltra3,
            Platform::Framework13Amd7080 => FrameworkPlatform::Framework13Amd7080,
            Platform::Framework13AmdAi300 => FrameworkPlatform::Framework13AmdAi300,
            Platform::Framework16Amd7080 => FrameworkPlatform::Framework16Amd7080,
            Platform::Framework16AmdAi300 => FrameworkPlatform::Framework16AmdAi300,
            Platform::FrameworkDesktopAmdAiMax300 => FrameworkPlatform::FrameworkDesktopAmdAiMax300,
            Platform::GenericFramework(..) => FrameworkPlatform::GenericFramework,
            Platform::UnknownSystem => FrameworkPlatform::UnknownSystem,
        }
    }
}

impl From<PlatformFamily> for FrameworkPlatformFamily {
    fn from(value: PlatformFamily) -> Self {
        match value {
            PlatformFamily::Framework12 => FrameworkPlatformFamily::Framework12,
            PlatformFamily::Framework13 => FrameworkPlatformFamily::Framework13,
            PlatformFamily::Framework16 => FrameworkPlatformFamily::Framework16,
            PlatformFamily::FrameworkDesktop => FrameworkPlatformFamily::FrameworkDesktop,
        }
    }
}

impl From<EcCurrentImage> for FrameworkEcCurrentImage {
    fn from(value: EcCurrentImage) -> Self {
        match value {
            EcCurrentImage::Unknown => FrameworkEcCurrentImage::Unknown,
            EcCurrentImage::RO => FrameworkEcCurrentImage::Ro,
            EcCurrentImage::RW => FrameworkEcCurrentImage::Rw,
        }
    }
}
