//! Simple UserAgent parser/stripper

use std::fmt;

use woothee::parser::Parser;

use crate::error::{HandlerErrorKind, HandlerResult};
use crate::tags::Tags;

/// ADM required browser format form
#[allow(dead_code)]
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
#[allow(dead_code)]
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

/* Currently unused
/// Strip a Firefox User-Agent string, returning a version only varying in Base
/// OS (e.g. Mac, Windows, Linux) and Firefox major version number
pub fn strip_ua(ua: &str) -> String {
    let WootheeResult {
        name, os, version, ..
    } = Parser::new().parse(ua).unwrap_or_default();

    let os = os.to_lowercase();
    let platform = match os.as_str() {
        _ if os.starts_with("windows") => "Windows NT 10.0; Win64; x64",
        "mac osx" => "Macintosh; Intel Mac OS X 10.15",
        "linux" => "X11; Ubuntu; Linux x86_64",
        _ => "Other",
    };
    let major = if name.to_lowercase().as_str() != "firefox" || version == VALUE_UNKNOWN {
        "?"
    } else {
        version.split('.').take(1).collect::<Vec<_>>()[0]
    };
    format!(
        "Mozilla/5.0 ({}; rv:{major}.0) Gecko/20100101 Firefox/{major}.0",
        platform,
        major = major
    )
}
*/

/// Convert a UserAgent header into a simplified ([OsFamily], [FormFactor])
pub fn get_device_info(ua: &str, tags: &mut Tags) -> HandlerResult<(OsFamily, FormFactor)> {
    let wresult = Parser::new().parse(ua).unwrap_or_default();

    // If it's not firefox, it doesn't belong here...
    if !["firefox"].contains(&wresult.name.to_lowercase().as_str()) {
        tags.add_extra("name", ua);
        return Err(HandlerErrorKind::InvalidUA().into());
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
    Ok((os_family, form_factor))
}

#[cfg(test)]
mod tests {
    use crate::error::HandlerErrorKind;
    use crate::tags::Tags;

    use super::{get_device_info, FormFactor, OsFamily};

    macro_rules! assert_strip_eq {
        ($value:expr, $stripped:expr) => {
            /* assert_eq!(strip_ua($value), $stripped); */
        };
    }

    macro_rules! assert_get_device_info {
        ($value:expr, $os_family:expr, $form_factor:expr) => {
            let mut tags = Tags::default();
            assert_eq!(
                get_device_info($value, &mut tags).expect("Error"),
                ($os_family, $form_factor)
            );
        };
    }

    #[test]
    fn macos() {
        assert_strip_eq!(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 11.2; rv:85.0) Gecko/20100101 Firefox/85.0",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:85.0) Gecko/20100101 Firefox/85.0"
        );
        assert_get_device_info!(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 11.2; rv:85.0) Gecko/20100101 Firefox/85.0",
            OsFamily::MacOs,
            FormFactor::Desktop
        );
    }

    #[test]
    fn windows() {
        assert_strip_eq!(
            "Mozilla/5.0 (Windows NT 6.1; Win64; x64; rv:61.0) Gecko/20100101 Firefox/61.0",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:61.0) Gecko/20100101 Firefox/61.0"
        );
        assert_get_device_info!(
            "Mozilla/5.0 (Windows NT 6.1; Win64; x64; rv:61.0) Gecko/20100101 Firefox/61.0",
            OsFamily::Windows,
            FormFactor::Desktop
        );
    }

    #[test]
    fn linux() {
        assert_strip_eq!(
            "Mozilla/5.0 (X11; Fedora; Linux x86_64; rv:82.0.1) Gecko/20100101 Firefox/82.0.1",
            "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:82.0) Gecko/20100101 Firefox/82.0"
        );
        assert_get_device_info!(
            "Mozilla/5.0 (X11; Fedora; Linux x86_64; rv:82.0.1) Gecko/20100101 Firefox/82.0.1",
            OsFamily::Linux,
            FormFactor::Desktop
        );
    }

    #[test]
    fn only_pass_major() {
        assert_strip_eq!(
            "Mozilla/5.0 (Windows NT 6.2; rv:78.6) Gecko/20100101 Firefox/78.6",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:78.0) Gecko/20100101 Firefox/78.0"
        );
    }

    #[test]
    fn other_os() {
        // don't even pass geckoversion (rv), only major
        assert_strip_eq!(
            "Mozilla/5.0 (Android 11; Mobile; rv:68.0) Gecko/68.0 Firefox/85.0",
            "Mozilla/5.0 (Other; rv:85.0) Gecko/20100101 Firefox/85.0"
        );
        assert_strip_eq!("", "Mozilla/5.0 (Other; rv:?.0) Gecko/20100101 Firefox/?.0");
    }

    #[test]
    fn android() {
        assert_get_device_info!(
            "Mozilla/5.0 (Android 11; Mobile; rv:68.0) Gecko/68.0 Firefox/85.0",
            OsFamily::Android,
            FormFactor::Phone
        );
    }

    #[test]
    fn ios() {
        assert_get_device_info!(
            "Mozilla/5.0 (iPad; CPU iPhone OS 8_3 like Mac OS X) AppleWebKit/600.1.4 (KHTML, like Gecko) FxiOS/1.0 Mobile/12F69 Safari/600.1.4",
            OsFamily::IOs,
            FormFactor::Tablet
        );
        assert_get_device_info!(
            "Mozilla/5.0 (iPhone; CPU iPhone OS 8_3 like Mac OS X) AppleWebKit/600.1.4 (KHTML, like Gecko) FxiOS/1.0 Mobile/12F69 Safari/600.1.4",
            OsFamily::IOs,
            FormFactor::Phone
        );
    }

    #[test]
    fn chromeos() {
        let mut tags = Tags::default();
        let result = get_device_info("Mozilla/5.0 (X11; CrOS x86_64 13816.64.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/90.0.4430.100 Safari/537.36", &mut tags);
        assert!(result.is_err());
        match result.unwrap_err().kind() {
            HandlerErrorKind::InvalidUA() => {}
            _ => panic!("Incorrect error returned for test"),
        }
    }

    #[test]
    fn other_ua() {
        let mut tags = Tags::default();
        assert_strip_eq!(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 11_2) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/88.0.4324.150 Safari/537.36",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:?.0) Gecko/20100101 Firefox/?.0"
        );
        assert!(get_device_info(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 11_2) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/88.0.4324.150 Safari/537.36",
            &mut tags)
                .is_err()
        );
    }
}
