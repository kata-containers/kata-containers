use slog::{Record, Serializer};

pub(crate) struct Visitor {
    kvs: Vec<(String, String)>,
}

impl Visitor {
    pub fn new() -> Self {
        Self { kvs: vec![] }
    }
}

impl<'kvs, 'a> log::kv::Visitor<'kvs> for Visitor {
    fn visit_pair(
        &mut self,
        key: log::kv::Key<'kvs>,
        val: log::kv::Value<'kvs>,
    ) -> Result<(), log::kv::Error> {
        let key = key.to_string();
        if let Some(val) = val.to_borrowed_str() {
            let val = val.to_string();
            self.kvs.push((key, val));
        }
        Ok(())
    }
}

impl slog::KV for Visitor {
    fn serialize(&self, _record: &Record, serializer: &mut dyn Serializer) -> slog::Result {
        for (key, val) in &self.kvs {
            serializer.emit_str(key.to_owned().into(), val.as_str())?;
        }
        Ok(())
    }
}
