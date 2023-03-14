use std::collections::BTreeMap;
use std::fmt::Display;
use std::io::Read;
use std::ops::Bound::Included;
use std::string::ToString;
use std::sync::Arc;

use anyhow::bail;

use crate::jwk::Jwk;
use crate::{JoseError, Map, Value};

/// Represents JWK set.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct JwkSet {
    keys: Vec<Arc<Jwk>>,
    params: Map<String, Value>,
    kid_map: BTreeMap<(String, usize), Arc<Jwk>>,
}

impl JwkSet {
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            params: Map::new(),
            kid_map: BTreeMap::new(),
        }
    }

    pub fn from_map(map: Map<String, Value>) -> Result<Self, JoseError> {
        (|| -> anyhow::Result<Self> {
            let mut kid_map = BTreeMap::new();
            let keys = match map.get("keys") {
                Some(Value::Array(vals)) => {
                    let mut vec = Vec::new();
                    for (i, val) in vals.iter().enumerate() {
                        match val {
                            Value::Object(val) => {
                                let jwk = Arc::new(Jwk::from_map(val.clone())?);
                                if let Some(kid) = jwk.key_id() {
                                    kid_map.insert((kid.to_string(), i), Arc::clone(&jwk));
                                }
                                vec.push(jwk);
                            }
                            _ => {
                                bail!("An element of the JWK set keys parameter must be a object.")
                            }
                        }
                    }
                    vec
                }
                Some(_) => bail!("The JWT keys parameter must be a array."),
                None => bail!("The JWK set must have a keys parameter."),
            };

            Ok(Self {
                keys,
                params: map,
                kid_map: kid_map,
            })
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJwkFormat(err),
        })
    }

    pub fn from_reader(input: &mut dyn Read) -> Result<Self, JoseError> {
        (|| -> anyhow::Result<Self> {
            let keys: Map<String, Value> = serde_json::from_reader(input)?;
            Ok(Self::from_map(keys)?)
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJwkFormat(err),
        })
    }

    pub fn from_bytes(input: impl AsRef<[u8]>) -> Result<Self, JoseError> {
        (|| -> anyhow::Result<Self> {
            let keys: Map<String, Value> = serde_json::from_slice(input.as_ref())?;
            Ok(Self::from_map(keys)?)
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJwkFormat(err),
        })
    }

    pub fn get(&self, key_id: &str) -> Vec<&Jwk> {
        let mut vec = Vec::new();
        for (_, val) in self.kid_map.range((
            Included((key_id.to_string(), 0)),
            Included((key_id.to_string(), usize::MAX)),
        )) {
            let jwk: &Jwk = &val;
            vec.push(jwk);
        }
        vec
    }

    pub fn keys(&self) -> Vec<&Jwk> {
        self.keys.iter().map(|e| e.as_ref()).collect()
    }

    pub fn push_key(&mut self, jwk: Jwk) {
        match self.params.get_mut("keys") {
            Some(Value::Array(keys)) => {
                keys.push(Value::Object(jwk.as_ref().clone()));
            }
            _ => unreachable!(),
        }

        let jwk = Arc::new(jwk);
        if let Some(kid) = jwk.key_id() {
            self.kid_map
                .insert((kid.to_string(), self.keys.len()), Arc::clone(&jwk));
        }
        self.keys.push(jwk);
    }

    pub fn remove_key(&mut self, jwk: &Jwk) {
        let index = self.keys.iter().position(|e| e.as_ref() == jwk);
        if let Some(index) = index {
            match self.params.get_mut("keys") {
                Some(Value::Array(keys)) => {
                    keys.remove(index);
                }
                _ => unreachable!(),
            }
            self.keys.remove(index);
        }
    }
}

impl AsRef<Map<String, Value>> for JwkSet {
    fn as_ref(&self) -> &Map<String, Value> {
        &self.params
    }
}

impl Into<Map<String, Value>> for JwkSet {
    fn into(self) -> Map<String, Value> {
        self.params
    }
}

impl Display for JwkSet {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.write_str("{\"keys\":[")?;

        for (i, jwk) in self.keys.iter().enumerate() {
            if i > 0 {
                fmt.write_str(",")?;
            }

            let map: &Map<String, Value> = &jwk.as_ref().as_ref();
            let val = serde_json::to_string(map).map_err(|_e| std::fmt::Error {})?;
            fmt.write_str(&val)?;
        }

        fmt.write_str("]}")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::fs::File;
    use std::path::PathBuf;

    #[test]
    fn test_load_jwt_set() -> Result<()> {
        let mut file = load_file("jwks/test.jwks")?;
        let jwks = JwkSet::from_reader(&mut file)?;

        assert_eq!(jwks.get("1").len(), 1);
        let key_id = jwks.get("1")[0].key_id();
        assert!(matches!(key_id, Some("1")));

        println!("{}", &jwks);

        Ok(())
    }

    fn load_file(path: &str) -> Result<File> {
        let mut pb = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pb.push("data");
        pb.push(path);

        let file = File::open(&pb)?;
        Ok(file)
    }
}
