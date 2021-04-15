use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::Arc;

use maxminddb::{self, geoip2::City, MaxMindDBError};
use serde::{self, Serialize};

use crate::error::{HandlerErrorKind, HandlerResult};
use crate::settings::Settings;

#[derive(Serialize, Debug, Default, Clone)]
pub struct LocationResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provinces: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dma: Option<u16>,
}

#[derive(Default, Clone)]
pub struct Location {
    iploc: Option<Arc<maxminddb::Reader<Vec<u8>>>>,
}

#[allow(unreachable_patterns)]
fn handle_mmdb_err(err: &MaxMindDBError) -> Option<HandlerErrorKind> {
    match err {
        MaxMindDBError::InvalidDatabaseError(s) => Some(HandlerErrorKind::Internal(format!(
            "Invalid GeoIP database! {:?}",
            s
        ))),
        MaxMindDBError::IoError(s) => Some(HandlerErrorKind::Location(format!(
            "Could not read GeoIP database: {:?}",
            s
        ))),
        MaxMindDBError::MapError(s) => Some(HandlerErrorKind::Location(format!(
            "GeoIP database mapping error: {:?}",
            s
        ))),
        MaxMindDBError::DecodingError(s) => {
            warn!("Could not decode GeoIP result: {:?}", s);
            None
        }
        MaxMindDBError::AddressNotFoundError(s) => {
            debug!("No entry in GeoIP database: {:?}", s);
            None
        }
        _ => Some(HandlerErrorKind::Location(format!(
            "Unknown GeoIP Error: {:?}",
            err
        ))),
    }
}

/// Generate a valid IP Lookup from the settings.
impl From<&Settings> for Location {
    fn from(settings: &Settings) -> Self {
        Self {
            iploc: settings.into(),
        }
    }
}

/// Create an mmdb reader from the settings.
impl From<&Settings> for Option<Arc<maxminddb::Reader<Vec<u8>>>> {
    fn from(settings: &Settings) -> Self {
        if let Some(path) = settings.maxminddb_loc.clone() {
            if !std::path::Path::new(&path).exists() {
                error!("Could not find mmdb database at {:?}", path);
                return None;
            }
            return Some(Arc::new(
                maxminddb::Reader::open_readfile(&path)
                    .unwrap_or_else(|_| panic!("Could read mmdb file at {:?}", path)),
            ));
        }
        None
    }
}

/// Parse the Accept-Language header to get the list of preferred languages.
///
/// We default to "en" because of well-established Anglo-biases.
pub fn preferred_languages(accepted_lang_header: String, default: &str) -> Vec<String> {
    let default_lang = String::from(default);
    let mut lang_tree: BTreeMap<String, String> = BTreeMap::new();
    let mut i = 0;
    accepted_lang_header.split(',').for_each(|l| {
        if l != "-" {
            if l.contains(';') {
                let weight: Vec<&str> = l.split(';').collect();
                let lang = weight[0].to_ascii_lowercase();
                let pref = weight[1].to_ascii_lowercase();
                lang_tree.insert(String::from(pref.trim()), String::from(lang.trim()));
            } else {
                lang_tree.insert(format!("q=1.{:02}", i), l.to_ascii_lowercase());
                i += 1;
            }
        }
    });
    let mut langs: Vec<String> = lang_tree
        .values()
        .map(std::borrow::ToOwned::to_owned)
        .collect();
    langs.reverse();
    langs.push(default_lang);
    langs
}

/// Return the element that most closely matches the preferred language.
/// This rounds up from the dialect if possible.
fn get_preferred_language_element(
    langs: &[String],
    elements: BTreeMap<&str, &str>,
) -> Option<String> {
    for lang in langs {
        // It's a wildcard, so just return the first possible choice.
        if lang == "*" || lang == "-" {
            return elements.values().next().map(|v| (*v).to_string());
        }
        if elements.contains_key(lang.as_str()) {
            if let Some(element) = elements.get(lang.as_str()) {
                return Some(element.to_string());
            }
        }
        if lang.contains('-') {
            let (lang, _) = lang.split_at(2);
            if elements.contains_key(lang) {
                if let Some(element) = elements.get(lang) {
                    return Some(element.to_string());
                }
            }
        }
    }
    None
}

/// IP address to Location
impl Location {
    /// Is the location look-up service available?
    pub fn is_available(&self) -> bool {
        self.iploc.is_some()
    }

    /// Resolve an `ip_addr` to a `LocationResult` using the `preferred_languages` as a hint for the language to use.
    ///
    /// `preferred_languages` is an array of `Accepted-Langauge` type pairs. You can use `preferred_languages` to
    /// convert the `Accepted-Language` header into this set.
    pub async fn locate(
        &self,
        ip_addr: IpAddr,
        preferred_languages: &[String],
    ) -> HandlerResult<Option<LocationResult>> {
        if self.iploc.is_none() {
            return Ok(None);
        }
        /*
        The structure of the returned maxminddb free record is:
        City:maxminddb::geoip::model::City {
            city: Some(City{
                geoname_id: Some(#),
                names: Some({"lang": "name", ...})
                }),
            continent: Some(Continent{
                geoname_id: Some(#),
                names: Some({...})
                }),
            country: Some(Country{
                geoname_id: Some(#),
                names: Some({...})
                }),
            location: Some(Location{
                latitude: Some(#.#),
                longitude: Some(#.#),
                metro_code: Some(#),
                time_zone: Some(".."),
                }),
            postal: Some(Postal {
                code: Some("..")
                }),
            registered_country: Some(Country {
                geoname_id: Some(#),
                iso_code: Some(".."),
                names: Some({"lang": "name", ...})
                }),
            represented_country: None,
            subdivisions: Some([Subdivision {
                geoname_id: Some(#),
                iso_code: Some(".."),
                names: Some({"lang": "name", ...})
                }]),
            traits: None }
        }
        */
        let mut result = LocationResult::default();
        match self.iploc.clone().unwrap().lookup::<City<'_>>(ip_addr) {
            Ok(location) => {
                if let Some(names) = location.city.and_then(|c| c.names) {
                    result.city = get_preferred_language_element(&preferred_languages, names)
                };
                if let Some(names) = location.country.and_then(|c| c.names) {
                    result.country = get_preferred_language_element(&preferred_languages, names)
                };
                if let Some(divs) = location.subdivisions {
                    let mut provinces = Vec::new();
                    for item in divs {
                        if let Some(names) = item.names {
                            if let Some(province) =
                                get_preferred_language_element(&preferred_languages, names)
                            {
                                provinces.push(province);
                            }
                        }
                    }
                    if !provinces.is_empty() {
                        result.provinces = Some(provinces);
                    }
                }
                if let Some(location) = location.location {
                    result.dma = location.metro_code;
                };
                return Ok(Some(result));
            }
            Err(err) => match handle_mmdb_err(&err) {
                Some(e) => return Err(e.into()),
                None => return Ok(None),
            },
        };
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::error::HandlerResult;
    use std::collections::BTreeMap;

    const MMDB_LOC: &str = "mmdb/GeoLite2-City.mmdb";
    #[test]
    fn test_preferred_language() {
        let langs = preferred_languages("en-US,es;q=0.1,en;q=0.5,*;q=0.2".to_owned(), "en");
        assert_eq!(
            vec![
                "en-us".to_owned(),
                "en".to_owned(),
                "*".to_owned(),
                "es".to_owned(),
                "en".to_owned(),
            ],
            langs
        );
    }

    #[test]
    fn test_bad_preferred_language() {
        let langs = preferred_languages("-".to_owned(), "en");
        assert_eq!(vec!["en".to_owned()], langs);
    }

    #[test]
    fn test_get_preferred_language_element() {
        let langs = vec![
            "en-us".to_owned(),
            "en".to_owned(),
            "es".to_owned(),
            "en".to_owned(),
        ];
        // Don't include the default "en" so we can test no matching languages.
        let bad_lang = vec!["fu".to_owned()];
        // Include the "*" so we can return any language.
        let any_lang = vec!["fu".to_owned(), "*".to_owned(), "en".to_owned()];
        let mut elements = BTreeMap::new();
        elements.insert("de", "Kalifornien");
        elements.insert("en", "California");
        elements.insert("fr", "Californie");
        elements.insert("ja", "カリフォルニア州");
        assert_eq!(
            Some("California".to_owned()),
            get_preferred_language_element(&langs, elements.clone())
        );
        assert_eq!(
            None,
            get_preferred_language_element(&bad_lang, elements.clone())
        );
        // Return Dutch, since it's the first key listed.
        assert!(get_preferred_language_element(&any_lang, elements.clone()).is_some());
        let goof_lang = vec!["🙄💩".to_owned()];
        assert_eq!(
            None,
            get_preferred_language_element(&goof_lang, elements.clone())
        );
    }

    #[actix_rt::test]
    async fn test_location_good() -> HandlerResult<()> {
        let test_ip: IpAddr = "63.245.208.195".parse().unwrap(); // Mozilla
        let langs = vec!["en".to_owned()];
        let mut settings = Settings::default();
        settings.maxminddb_loc = Some(MMDB_LOC.to_owned());
        let location = Location::from(&settings);
        if location.is_available() {
            // TODO: either mock maxminddb::Reader or pass it in as a wrapped impl
            let result = location.locate(test_ip, &langs).await?.unwrap();
            assert_eq!(result.city, Some("Sacramento".to_owned()));
            assert_eq!(result.provinces, Some(["California".to_owned()].to_vec()));
            assert_eq!(result.country, Some("United States".to_owned()));
        } else {
            println!("⚠Location Database not found, cannot test location, skipping");
        }
        Ok(())
    }

    #[actix_rt::test]
    async fn test_location_bad() -> HandlerResult<()> {
        let test_ip: IpAddr = "192.168.1.1".parse().unwrap();
        let langs = vec!["en".to_owned()];
        let mut settings = Settings::default();
        settings.maxminddb_loc = Some(MMDB_LOC.to_owned());
        let location = Location::from(&settings);
        if location.is_available() {
            let result = location.locate(test_ip, &langs).await?;
            assert!(result.is_none());
        } else {
            println!("⚠Location Database not found, cannot test location, skipping");
        }
        Ok(())
    }
}
