use woothee::parser::Parser;

/// Strip a Firefox User-Agent string, returning a version only varying in Base
/// OS (e.g. Mac, Windows, Linux) and Firefox major version number
pub fn strip_ua(ua: &str) -> String {
    let parser = Parser::new();
    let wresult = parser.parse(ua).unwrap_or_default();

    let platform = if wresult.os.starts_with("Windows") {
        "Windows NT 10.0; Win64; x64"
    } else {
        match wresult.os {
            "Mac OSX" => "Macintosh; Intel Mac OS X 10.15",
            "Linux" => "X11; Ubuntu; Linux x86_64",
            _ => "Other",
        }
    };
    let major = wresult.version.split('.').take(1).collect::<Vec<_>>()[0];
    format!(
        "Mozilla/5.0 ({}; rv:{major}.0) Gecko/20100101 Firefox/{major}.0",
        platform,
        major = major
    )
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
    fn other() {
        // don't even pass geckoversion (rv), only major
        assert_strip_eq!(
            "Mozilla/5.0 (Android 11; Mobile; rv:68.0) Gecko/68.0 Firefox/85.0",
            "Mozilla/5.0 (Other; rv:85.0) Gecko/20100101 Firefox/85.0"
        );
    }
}
