use chrono::offset::Utc;
use chrono::DateTime;
use semver::Version;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
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
// 2017-06-20 17:30:52.919673
#[derive(Deserialize, Debug)]
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
    pub fn krate(&self) -> &CrateRecord {
        &self.krate
    }

    pub fn version(&self) -> &Vec<VersionRecord> {
        &self.versions
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
