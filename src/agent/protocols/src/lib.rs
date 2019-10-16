#![allow(bare_trait_objects)]

pub mod agent;
pub mod agent_grpc;
pub mod health;
pub mod health_grpc;
pub mod oci;
pub mod types;
pub mod empty;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
