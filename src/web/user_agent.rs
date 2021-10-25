//! Simple UserAgent parser/stripper

use std::fmt;
use std::str::FromStr;

use woothee::parser::Parser;

use crate::error::{HandlerError, HandlerErrorKind, HandlerResult};

/// ADM required browser format form
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FormFactor {
    Desktop,
    Phone,
    Tablet,
    Other,
}

impl fmt::Display for FormFactor {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = format!("{:?}", self).to_lowercase();
        write!(fmt, "{}", name)
    }
}

/// Simplified Operating System Family
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OsFamily {
    Windows,
    MacOs,
    Linux,
    IOs,
    Android,
    ChromeOs,
    BlackBerry,
    Other,
}

impl fmt::Display for OsFamily {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        // XXX: could use "correct" case (rendering this w/ serde will make
        // that easier)
        let name = format!("{:?}", self).to_lowercase();
        write!(fmt, "{}", name)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct DeviceInfo {
    pub form_factor: FormFactor,
    pub os_family: OsFamily,
    // We only care about major versions.
    pub ff_version: u32,
}

impl DeviceInfo {
    pub fn legacy_only(&self) -> bool {
        self.os_family == OsFamily::IOs && self.ff_version < 36
            || self.os_family != OsFamily::IOs && self.ff_version < 91
    }
}

/// Parse a User-Agent header into a simplified `DeviceInfo`
pub fn get_device_info(ua: &str) -> HandlerResult<DeviceInfo> {
    let wresult = Parser::new().parse(ua).unwrap_or_default();

    // If it's not firefox, it doesn't belong here...
    if !["firefox"].contains(&wresult.name.to_lowercase().as_str()) {
        let mut err: HandlerError = HandlerErrorKind::InvalidUA.into();
        // XXX: Tags::from_head already adds this
        err.tags.add_extra("ua", ua);
        err.tags
            .add_extra("name", wresult.name.to_lowercase().as_str());
        return Err(err);
    }

    let os = wresult.os.to_lowercase();
    let os_family = match os.as_str() {
        _ if os.starts_with("windows") => OsFamily::Windows,
        "mac osx" => OsFamily::MacOs,
        "linux" => OsFamily::Linux,
        "iphone" | "ipad" => OsFamily::IOs,
        "android" => OsFamily::Android,
        "chromeos" => OsFamily::ChromeOs,
        "blackberry" => OsFamily::BlackBerry,
        _ => OsFamily::Other,
    };
    let form_factor = match wresult.category {
        "pc" => FormFactor::Desktop,
        "smartphone" if os.as_str() == "ipad" => FormFactor::Tablet,
        "smartphone" => FormFactor::Phone,
        _ => FormFactor::Other,
    };

    let ff_version =
        u32::from_str(wresult.version.split('.').collect::<Vec<&str>>()[0]).unwrap_or_default();
    Ok(DeviceInfo {
        form_factor,
        os_family,
        ff_version,
    })
}

#[cfg(test)]
mod tests {
    use crate::error::HandlerErrorKind;

    use super::{get_device_info, DeviceInfo, FormFactor, OsFamily};

    macro_rules! assert_get_device_info {
        ($value:expr, $os_family:expr, $form_factor:expr, $ff_version:expr) => {
            assert_eq!(
                get_device_info($value).expect("Error"),
                DeviceInfo {
                    os_family: $os_family,
                    form_factor: $form_factor,
                    ff_version: $ff_version,
                }
            );
        };
    }

    #[test]
    fn macos() {
        assert_get_device_info!(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 11.2; rv:85.0) Gecko/20100101 Firefox/85.0",
            OsFamily::MacOs,
            FormFactor::Desktop,
            85
        );
    }

    #[test]
    fn windows() {
        assert_get_device_info!(
            "Mozilla/5.0 (Windows NT 6.1; Win64; x64; rv:61.0) Gecko/20100101 Firefox/61.0",
            OsFamily::Windows,
            FormFactor::Desktop,
            61
        );
    }

    #[test]
    fn linux() {
        assert_get_device_info!(
            "Mozilla/5.0 (X11; Fedora; Linux x86_64; rv:82.0.1) Gecko/20100101 Firefox/82.0.1",
            OsFamily::Linux,
            FormFactor::Desktop,
            82
        );
    }

    #[test]
    fn android() {
        assert_get_device_info!(
            "Mozilla/5.0 (Android 11; Mobile; rv:68.0) Gecko/68.0 Firefox/85.0",
            OsFamily::Android,
            FormFactor::Phone,
            85
        );
    }

    #[test]
    fn ios() {
        assert_get_device_info!(
            "Mozilla/5.0 (iPad; CPU iPhone OS 8_3 like Mac OS X) AppleWebKit/600.1.4 (KHTML, like Gecko) FxiOS/1.0 Mobile/12F69 Safari/600.1.4",
            OsFamily::IOs,
            FormFactor::Tablet,
            1
        );
        assert_get_device_info!(
            "Mozilla/5.0 (iPhone; CPU iPhone OS 8_3 like Mac OS X) AppleWebKit/600.1.4 (KHTML, like Gecko) FxiOS/2.0 Mobile/12F69 Safari/600.1.4",
            OsFamily::IOs,
            FormFactor::Phone,
            2
        );
    }

    #[test]
    fn chromeos() {
        let ua_str = "Mozilla/5.0 (X11; CrOS x86_64 13816.64.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/90.0.4430.100 Safari/537.36";
        let result = get_device_info(ua_str);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err.kind() {
            HandlerErrorKind::InvalidUA => {}
            _ => panic!("Incorrect error returned for test"),
        }
        assert!(err.tags.extra.get("ua") == Some(&ua_str.to_owned()));
        assert!(err.tags.extra.get("name") == Some(&"chrome".to_owned()));
        dbg!(err.tags);
    }

    #[test]
    fn other_ua() {
        assert!(get_device_info(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 11_2) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/88.0.4324.150 Safari/537.36")
                .is_err()
        );
    }
}
