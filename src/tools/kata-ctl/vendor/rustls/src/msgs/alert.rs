use crate::msgs::codec::{Codec, Reader};
use crate::msgs::enums::{AlertDescription, AlertLevel};

#[derive(Debug)]
pub struct AlertMessagePayload {
    pub level: AlertLevel,
    pub description: AlertDescription,
}

impl Codec for AlertMessagePayload {
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.level.encode(bytes);
        self.description.encode(bytes);
    }

    fn read(r: &mut Reader) -> Option<Self> {
        let level = AlertLevel::read(r)?;
        let description = AlertDescription::read(r)?;

        Some(Self { level, description })
    }
}
