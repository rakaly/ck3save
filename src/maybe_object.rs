use std::fmt;
use std::marker::PhantomData;
use serde::{Deserialize, Deserializer, de, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum MaybeObject<T> {
    Text(String),
    Object(T),
}

impl<'de, T> Deserialize<'de> for MaybeObject<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MaybeObjectVisitor<T1> {
            marker: PhantomData<T1>,
        }

        impl<'de, T1> de::Visitor<'de> for MaybeObjectVisitor<T1>
        where
            T1: Deserialize<'de>,
        {
            type Value = MaybeObject<T1>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an object or string")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(MaybeObject::Text(String::from(v)))
            }

            fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mvd = de::value::MapAccessDeserializer::new(map);
                let result = T1::deserialize(mvd)?;
                Ok(MaybeObject::Object(result))
            }
        }

        deserializer.deserialize_map(MaybeObjectVisitor {
            marker: PhantomData,
        })
    }
}
