use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::str::FromStr;

use clap::ValueEnum;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

// NOTE: Copilot IDs are opaque runtime strings. We use a marker type per ID kind so they
// are not accidentally interchangeable (similar to Swift's Tagged<>).
pub struct OwnedId<T> {
    raw: String,
    _marker: PhantomData<fn() -> T>,
}

impl<T> OwnedId<T> {
    pub fn new(raw: String) -> Self {
        Self {
            raw,
            _marker: PhantomData,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.raw
    }
}

impl<T> Clone for OwnedId<T> {
    fn clone(&self) -> Self {
        Self::new(self.raw.clone())
    }
}

impl<T> fmt::Debug for OwnedId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Id").field(&self.raw).finish()
    }
}

impl<T> fmt::Display for OwnedId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl<T> PartialEq for OwnedId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}
impl<T> Eq for OwnedId<T> {}

impl<T> Hash for OwnedId<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl<T> FromStr for OwnedId<T> {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s.to_string()))
    }
}

impl<T> From<String> for OwnedId<T> {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl<T> From<&str> for OwnedId<T> {
    fn from(value: &str) -> Self {
        Self::new(value.to_string())
    }
}

impl<T> Serialize for OwnedId<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.raw)
    }
}

impl<'de, T> Deserialize<'de> for OwnedId<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::new(s))
    }
}

pub enum TransactionMarker {}
pub enum CategoryMarker {}
pub enum TagMarker {}
pub enum RecurringMarker {}
pub enum AccountMarker {}
pub enum ItemMarker {}

pub type TransactionId = OwnedId<TransactionMarker>;
pub type CategoryId = OwnedId<CategoryMarker>;
pub type TagId = OwnedId<TagMarker>;
pub type RecurringId = OwnedId<RecurringMarker>;
pub type AccountId = OwnedId<AccountMarker>;
pub type ItemId = OwnedId<ItemMarker>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransactionType {
    Regular,
    InternalTransfer,
    #[serde(other)]
    Other,
}

impl fmt::Display for TransactionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            TransactionType::Regular => "REGULAR",
            TransactionType::InternalTransfer => "INTERNAL_TRANSFER",
            TransactionType::Other => "OTHER",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecurringFrequency {
    Daily,
    Weekly,
    Biweekly,
    Monthly,
    Quarterly,
    Annually,
    #[serde(other)]
    Other,
}

impl fmt::Display for RecurringFrequency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            RecurringFrequency::Daily => "DAILY",
            RecurringFrequency::Weekly => "WEEKLY",
            RecurringFrequency::Biweekly => "BIWEEKLY",
            RecurringFrequency::Monthly => "MONTHLY",
            RecurringFrequency::Quarterly => "QUARTERLY",
            RecurringFrequency::Annually => "ANNUALLY",
            RecurringFrequency::Other => "OTHER",
        };
        write!(f, "{s}")
    }
}
