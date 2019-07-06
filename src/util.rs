use std::fmt;
use std::fmt::Display;
use std::str::FromStr;
use std::iter::FromIterator;
use std::marker::PhantomData;
use serde::{Deserializer, de};

// adapted from: https://github.com/serde-rs/serde/issues/581#issuecomment-253626616
pub fn space_separated<'de, V, T, D>(deserializer: D) -> Result<V, D::Error>
where
    V: FromIterator<T>,
    T: FromStr,
    T::Err: Display,
    D: Deserializer<'de>,
{
    struct SpaceSeparated<V, T>(PhantomData<V>, PhantomData<T>);

    impl<'de, V, T> de::Visitor<'de> for SpaceSeparated<V, T>
    where
        V: FromIterator<T>,
        T: FromStr,
        T::Err: Display,
    {
        type Value = V;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("string containing space-separated elements")
        }

        fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let iter = s.split(' ').map(FromStr::from_str);
            Result::from_iter(iter).map_err(de::Error::custom)
        }
    }

    let visitor = SpaceSeparated(PhantomData, PhantomData);
    deserializer.deserialize_str(visitor)
}
