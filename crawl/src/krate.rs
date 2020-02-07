use chrono::offset::Utc;
use chrono::DateTime;
use semver::Version;
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
pub struct CrateRecord {
    pub description: String,
    pub documentation: String,
    pub downloads: u64,
    pub homepage: String,
    pub id: u64,
    pub name: String,
    pub repository: String,
    #[serde(with = "psql_format")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "psql_format")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct VersionRecord {
    pub crate_id: u64,
    pub downloads: u64,
    pub id: u64,
    pub num: Version,
    #[serde(with = "psql_format")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "psql_format")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct Crate {
    krate: CrateRecord,
    versions: Vec<VersionRecord>,
}

impl Crate {
    pub fn new(krate: CrateRecord, versions: Vec<VersionRecord>) -> Crate {
        Crate { krate, versions }
    }

    pub fn name(&self) -> &str {
        &self.krate.name
    }

    pub fn downloads(&self) -> u64 {
        self.krate.downloads
    }

    pub fn id(&self) -> u64 {
        self.krate.id
    }

    pub fn krate(&self) -> &CrateRecord {
        &self.krate
    }

    pub fn versions(&self) -> &Vec<VersionRecord> {
        &self.versions
    }

    pub fn latest_version_record(&self) -> &VersionRecord {
        self.versions()
            .iter()
            .max_by_key(|record| &record.num)
            .unwrap()
    }

    /// Returns the latest version as `$CRATE_NAME-$CRATE_VERSION` form.
    /// Example: `crux-0.1.0`.
    pub fn latest_version_tag(&self) -> String {
        let record = self.latest_version_record();
        format!("{}-{}", self.krate.name, record.num)
    }
}

mod psql_format {
    use chrono::{DateTime, TimeZone, Utc};
    use serde::{self, Deserialize, Deserializer};

    const FORMAT: &'static str = "%Y-%m-%d %H:%M:%S%.6f";

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Utc.datetime_from_str(&s, FORMAT)
            .map_err(serde::de::Error::custom)
    }
}
