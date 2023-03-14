use slog::*;
use std::{fmt, result};

pub struct PrintlnSerializer;

impl Serializer for PrintlnSerializer {
    fn emit_arguments(&mut self, key: Key, val: &fmt::Arguments) -> Result {
        print!(", {}={}", key, val);
        Ok(())
    }
}

pub struct PrintlnDrain;

impl Drain for PrintlnDrain {
    type Ok = ();
    type Err = ();

    fn log(
        &self,
        record: &Record,
        values: &OwnedKVList,
    ) -> result::Result<Self::Ok, Self::Err> {

        print!("{}", record.msg());

        record
            .kv()
            .serialize(record, &mut PrintlnSerializer)
            .unwrap();
        values.serialize(record, &mut PrintlnSerializer).unwrap();

        println!("");
        Ok(())
    }
}
