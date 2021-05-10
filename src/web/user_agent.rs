use std::fmt;

use woothee::{
    parser::{Parser, WootheeResult},
    woothee::VALUE_UNKNOWN,
};

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

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OsFamily {
    Windows,
    Mac,
    Linux,
    Ios,
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

/// Strip a Firefox User-Agent string, returning a version only varying in Base
/// OS (e.g. Mac, Windows, Linux) and Firefox major version number
pub fn strip_ua(ua: &str) -> String {
    let WootheeResult {
        name, os, version, ..
    } = Parser::new().parse(ua).unwrap_or_default();

    let platform = match os {
        _ if os.starts_with("Windows") => "Windows NT 10.0; Win64; x64",
        "Mac OSX" => "Macintosh; Intel Mac OS X 10.15",
        "Linux" => "X11; Ubuntu; Linux x86_64",
        _ => "Other",
    };
    let major = if name != "Firefox" || version == VALUE_UNKNOWN {
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

pub fn get_device_info(ua: &str) -> (OsFamily, FormFactor) {
    let WootheeResult { os, .. } = Parser::new().parse(ua).unwrap_or_default();

    let os_family = match os {
        _ if os.starts_with("Windows") => OsFamily::Windows,
        "Mac OSX" => OsFamily::Mac,
        "Linux" => OsFamily::Linux,
        _ => OsFamily::Other,
    };
    let form_factor = if matches!(os_family, OsFamily::Other) {
        FormFactor::Other
    } else {
        FormFactor::Desktop
    };
    (os_family, form_factor)
}

#[cfg(test)]
mod tests {
    use super::strip_ua;

    macro_rules! assert_strip_eq {
        ($value:expr, $stripped:expr) => {
            assert_eq!(strip_ua($value), $stripped);
        };
    }

    #[test]
    fn macos() {
        assert_strip_eq!(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 11.2; rv:85.0) Gecko/20100101 Firefox/85.0",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:85.0) Gecko/20100101 Firefox/85.0"
        );
    }

    #[test]
    fn windows() {
        assert_strip_eq!(
            "Mozilla/5.0 (Windows NT 6.1; Win64; x64; rv:61.0) Gecko/20100101 Firefox/61.0",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:61.0) Gecko/20100101 Firefox/61.0"
        );
    }

    #[test]
    fn linux() {
        assert_strip_eq!(
            "Mozilla/5.0 (X11; Fedora; Linux x86_64; rv:82.0.1) Gecko/20100101 Firefox/82.0.1",
            "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:82.0) Gecko/20100101 Firefox/82.0"
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
    fn other_ua() {
        assert_strip_eq!(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 11_2) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/88.0.4324.150 Safari/537.36",
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:?.0) Gecko/20100101 Firefox/?.0"
        );
    }
}
