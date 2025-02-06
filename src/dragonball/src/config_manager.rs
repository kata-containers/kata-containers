// Copyright (C) 2020-2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::convert::TryInto;
use std::io;
use std::ops::{Index, IndexMut};
use std::sync::Arc;

use dbs_device::DeviceIo;
use dbs_utils::rate_limiter::{RateLimiter, TokenBucket};
use serde_derive::{Deserialize, Serialize};

/// Get bucket update for rate limiter.
#[macro_export]
macro_rules! get_bucket_update {
    ($self:ident, $rate_limiter: ident, $metric: ident) => {{
        match &$self.$rate_limiter {
            Some(rl_cfg) => {
                let tb_cfg = &rl_cfg.$metric;
                dbs_utils::rate_limiter::RateLimiter::make_bucket(
                    tb_cfg.size,
                    tb_cfg.one_time_burst,
                    tb_cfg.refill_time,
                )
                // Updated active rate-limiter.
                .map(dbs_utils::rate_limiter::BucketUpdate::Update)
                // Updated/deactivated rate-limiter
                .unwrap_or(dbs_utils::rate_limiter::BucketUpdate::Disabled)
            }
            // No update to the rate-limiter.
            None => dbs_utils::rate_limiter::BucketUpdate::None,
        }
    }};
}

/// Trait for generic configuration information.
pub trait ConfigItem {
    /// Related errors.
    type Err;

    /// Get the unique identifier of the configuration item.
    fn id(&self) -> &str;

    /// Check whether current configuration item conflicts with another one.
    fn check_conflicts(&self, other: &Self) -> std::result::Result<(), Self::Err>;
}

/// Struct to manage a group of configuration items.
#[derive(Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct ConfigInfos<T>
where
    T: ConfigItem + Clone,
{
    configs: Vec<T>,
}

impl<T> ConfigInfos<T>
where
    T: ConfigItem + Clone + Default,
{
    /// Constructor
    pub fn new() -> Self {
        ConfigInfos::default()
    }

    /// Insert a configuration item in the group.
    pub fn insert(&mut self, config: T) -> std::result::Result<(), T::Err> {
        for item in self.configs.iter() {
            config.check_conflicts(item)?;
        }
        self.configs.push(config);

        Ok(())
    }

    /// Update a configuration item in the group.
    pub fn update(&mut self, config: T, err: T::Err) -> std::result::Result<(), T::Err> {
        match self.get_index_by_id(&config) {
            None => Err(err),
            Some(index) => {
                for (idx, item) in self.configs.iter().enumerate() {
                    if idx != index {
                        config.check_conflicts(item)?;
                    }
                }
                self.configs[index] = config;
                Ok(())
            }
        }
    }

    /// Insert or update a configuration item in the group.
    pub fn insert_or_update(&mut self, config: T) -> std::result::Result<(), T::Err> {
        match self.get_index_by_id(&config) {
            None => {
                for item in self.configs.iter() {
                    config.check_conflicts(item)?;
                }

                self.configs.push(config)
            }
            Some(index) => {
                for (idx, item) in self.configs.iter().enumerate() {
                    if idx != index {
                        config.check_conflicts(item)?;
                    }
                }
                self.configs[index] = config;
            }
        }

        Ok(())
    }

    /// Remove the matching configuration entry.
    pub fn remove(&mut self, config: &T) -> Option<T> {
        if let Some(index) = self.get_index_by_id(config) {
            Some(self.configs.remove(index))
        } else {
            None
        }
    }

    /// Returns an immutable iterator over the config items
    pub fn iter(&self) -> ::std::slice::Iter<T> {
        self.configs.iter()
    }

    /// Get the configuration entry with matching ID.
    pub fn get_by_id(&self, item: &T) -> Option<&T> {
        let id = item.id();

        self.configs.iter().rfind(|cfg| cfg.id() == id)
    }

    fn get_index_by_id(&self, item: &T) -> Option<usize> {
        let id = item.id();
        self.configs.iter().position(|cfg| cfg.id() == id)
    }
}

impl<T> Clone for ConfigInfos<T>
where
    T: ConfigItem + Clone,
{
    fn clone(&self) -> Self {
        ConfigInfos {
            configs: self.configs.clone(),
        }
    }
}

/// Struct to maintain configuration information for a device.
pub struct DeviceConfigInfo<T>
where
    T: ConfigItem + Clone,
{
    /// Configuration information for the device object.
    pub config: T,
    /// The associated device object.
    pub device: Option<Arc<dyn DeviceIo>>,
}

impl<T> DeviceConfigInfo<T>
where
    T: ConfigItem + Clone,
{
    /// Create a new instance of ['DeviceConfigInfo'].
    pub fn new(config: T) -> Self {
        DeviceConfigInfo {
            config,
            device: None,
        }
    }

    /// Create a new instance of ['DeviceConfigInfo'] with optional device.
    pub fn new_with_device(config: T, device: Option<Arc<dyn DeviceIo>>) -> Self {
        DeviceConfigInfo { config, device }
    }

    /// Set the device object associated with the configuration.
    pub fn set_device(&mut self, device: Arc<dyn DeviceIo>) {
        self.device = Some(device);
    }
}

impl<T> Clone for DeviceConfigInfo<T>
where
    T: ConfigItem + Clone,
{
    fn clone(&self) -> Self {
        DeviceConfigInfo::new_with_device(self.config.clone(), self.device.clone())
    }
}

/// Struct to maintain configuration information for a group of devices.
pub struct DeviceConfigInfos<T>
where
    T: ConfigItem + Clone,
{
    info_list: Vec<DeviceConfigInfo<T>>,
}

impl<T> Default for DeviceConfigInfos<T>
where
    T: ConfigItem + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> DeviceConfigInfos<T>
where
    T: ConfigItem + Clone,
{
    /// Create a new instance of ['DeviceConfigInfos'].
    pub fn new() -> Self {
        DeviceConfigInfos {
            info_list: Vec::new(),
        }
    }

    /// Insert or update configuration information for a device.
    pub fn insert_or_update(&mut self, config: &T) -> std::result::Result<usize, T::Err> {
        let device_info = DeviceConfigInfo::new(config.clone());
        Ok(match self.get_index_by_id(config) {
            Some(index) => {
                for (idx, info) in self.info_list.iter().enumerate() {
                    if idx != index {
                        info.config.check_conflicts(config)?;
                    }
                }
                self.info_list[index].config = config.clone();
                index
            }
            None => {
                for info in self.info_list.iter() {
                    info.config.check_conflicts(config)?;
                }
                self.info_list.push(device_info);
                self.info_list.len() - 1
            }
        })
    }

    /// Remove a device configuration information object.
    pub fn remove(&mut self, index: usize) -> Option<DeviceConfigInfo<T>> {
        if self.info_list.len() > index {
            Some(self.info_list.remove(index))
        } else {
            None
        }
    }

    /// Get number of device configuration information objects.
    pub fn len(&self) -> usize {
        self.info_list.len()
    }

    /// Returns true if the device configuration information objects is empty.
    pub fn is_empty(&self) -> bool {
        self.info_list.len() == 0
    }

    /// Add a device configuration information object at the tail.
    pub fn push(&mut self, info: DeviceConfigInfo<T>) {
        self.info_list.push(info);
    }

    /// Iterator for configuration information objects.
    pub fn iter(&self) -> std::slice::Iter<DeviceConfigInfo<T>> {
        self.info_list.iter()
    }

    /// Mutable iterator for configuration information objects.
    pub fn iter_mut(&mut self) -> std::slice::IterMut<DeviceConfigInfo<T>> {
        self.info_list.iter_mut()
    }

    /// Remove the last device config info from the `info_list`.
    pub fn pop(&mut self) -> Option<DeviceConfigInfo<T>> {
        self.info_list.pop()
    }

    fn get_index_by_id(&self, config: &T) -> Option<usize> {
        self.info_list
            .iter()
            .position(|info| info.config.id().eq(config.id()))
    }
}

impl<T> Index<usize> for DeviceConfigInfos<T>
where
    T: ConfigItem + Clone,
{
    type Output = DeviceConfigInfo<T>;
    fn index(&self, idx: usize) -> &Self::Output {
        &self.info_list[idx]
    }
}

impl<T> IndexMut<usize> for DeviceConfigInfos<T>
where
    T: ConfigItem + Clone,
{
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        &mut self.info_list[idx]
    }
}

impl<T> Clone for DeviceConfigInfos<T>
where
    T: ConfigItem + Clone,
{
    fn clone(&self) -> Self {
        DeviceConfigInfos {
            info_list: self.info_list.clone(),
        }
    }
}

/// Configuration information for RateLimiter token bucket.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct TokenBucketConfigInfo {
    /// The size for the token bucket. A TokenBucket of `size` total capacity will take `refill_time`
    /// milliseconds to go from zero tokens to total capacity.
    pub size: u64,
    /// Number of free initial tokens, that can be consumed at no cost.
    pub one_time_burst: u64,
    /// Complete refill time in milliseconds.
    pub refill_time: u64,
}

impl TokenBucketConfigInfo {
    fn resize(&mut self, n: u64) {
        if n != 0 {
            self.size /= n;
            self.one_time_burst /= n;
        }
    }
}

impl From<TokenBucketConfigInfo> for TokenBucket {
    fn from(t: TokenBucketConfigInfo) -> TokenBucket {
        (&t).into()
    }
}

impl From<&TokenBucketConfigInfo> for TokenBucket {
    fn from(t: &TokenBucketConfigInfo) -> TokenBucket {
        TokenBucket::new(t.size, t.one_time_burst, t.refill_time)
    }
}

/// Configuration information for RateLimiter objects.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct RateLimiterConfigInfo {
    /// Data used to initialize the RateLimiter::bandwidth bucket.
    pub bandwidth: TokenBucketConfigInfo,
    /// Data used to initialize the RateLimiter::ops bucket.
    pub ops: TokenBucketConfigInfo,
}

impl RateLimiterConfigInfo {
    /// Update the bandwidth budget configuration.
    pub fn update_bandwidth(&mut self, new_config: TokenBucketConfigInfo) {
        self.bandwidth = new_config;
    }

    /// Update the ops budget configuration.
    pub fn update_ops(&mut self, new_config: TokenBucketConfigInfo) {
        self.ops = new_config;
    }

    /// resize the limiter to its 1/n.
    pub fn resize(&mut self, n: u64) {
        self.bandwidth.resize(n);
        self.ops.resize(n);
    }
}

impl TryInto<RateLimiter> for &RateLimiterConfigInfo {
    type Error = io::Error;

    fn try_into(self) -> Result<RateLimiter, Self::Error> {
        RateLimiter::new(
            self.bandwidth.size,
            self.bandwidth.one_time_burst,
            self.bandwidth.refill_time,
            self.ops.size,
            self.ops.one_time_burst,
            self.ops.refill_time,
        )
    }
}

impl TryInto<RateLimiter> for RateLimiterConfigInfo {
    type Error = io::Error;

    fn try_into(self) -> Result<RateLimiter, Self::Error> {
        RateLimiter::new(
            self.bandwidth.size,
            self.bandwidth.one_time_burst,
            self.bandwidth.refill_time,
            self.ops.size,
            self.ops.one_time_burst,
            self.ops.refill_time,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    pub enum DummyError {
        #[error("configuration entry exists")]
        Exist,
    }

    #[derive(Clone, Debug, Default)]
    pub struct DummyConfigInfo {
        id: String,
        content: String,
    }

    impl ConfigItem for DummyConfigInfo {
        type Err = DummyError;

        fn id(&self) -> &str {
            &self.id
        }

        fn check_conflicts(&self, other: &Self) -> Result<(), DummyError> {
            if self.id == other.id || self.content == other.content {
                Err(DummyError::Exist)
            } else {
                Ok(())
            }
        }
    }

    type DummyConfigInfos = ConfigInfos<DummyConfigInfo>;

    #[test]
    fn test_insert_config_info() {
        let mut configs = DummyConfigInfos::new();

        let config1 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "a".to_owned(),
        };
        configs.insert(config1).unwrap();
        assert_eq!(configs.configs.len(), 1);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "a");

        // Test case: cannot insert new item with the same id.
        let config2 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "b".to_owned(),
        };
        configs.insert(config2).unwrap_err();
        assert_eq!(configs.configs.len(), 1);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "a");

        let config3 = DummyConfigInfo {
            id: "2".to_owned(),
            content: "c".to_owned(),
        };
        configs.insert(config3).unwrap();
        assert_eq!(configs.configs.len(), 2);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "a");
        assert_eq!(configs.configs[1].id, "2");
        assert_eq!(configs.configs[1].content, "c");

        // Test case: cannot insert new item with the same content.
        let config4 = DummyConfigInfo {
            id: "3".to_owned(),
            content: "c".to_owned(),
        };
        configs.insert(config4).unwrap_err();
        assert_eq!(configs.configs.len(), 2);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "a");
        assert_eq!(configs.configs[1].id, "2");
        assert_eq!(configs.configs[1].content, "c");
    }

    #[test]
    fn test_update_config_info() {
        let mut configs = DummyConfigInfos::new();

        let config1 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "a".to_owned(),
        };
        configs.insert(config1).unwrap();
        assert_eq!(configs.configs.len(), 1);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "a");

        // Test case: succeed to update an existing entry
        let config2 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "b".to_owned(),
        };
        configs.update(config2, DummyError::Exist).unwrap();
        assert_eq!(configs.configs.len(), 1);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "b");

        // Test case: cannot update a non-existing entry
        let config3 = DummyConfigInfo {
            id: "2".to_owned(),
            content: "c".to_owned(),
        };
        configs.update(config3, DummyError::Exist).unwrap_err();
        assert_eq!(configs.configs.len(), 1);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "b");

        // Test case: cannot update an entry with conflicting content
        let config4 = DummyConfigInfo {
            id: "2".to_owned(),
            content: "c".to_owned(),
        };
        configs.insert(config4).unwrap();
        let config5 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "c".to_owned(),
        };
        configs.update(config5, DummyError::Exist).unwrap_err();
    }

    #[test]
    fn test_insert_or_update_config_info() {
        let mut configs = DummyConfigInfos::new();

        let config1 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "a".to_owned(),
        };
        configs.insert_or_update(config1).unwrap();
        assert_eq!(configs.configs.len(), 1);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "a");

        // Test case: succeed to update an existing entry
        let config2 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "b".to_owned(),
        };
        configs.insert_or_update(config2.clone()).unwrap();
        assert_eq!(configs.configs.len(), 1);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "b");

        // Add a second entry
        let config3 = DummyConfigInfo {
            id: "2".to_owned(),
            content: "c".to_owned(),
        };
        configs.insert_or_update(config3.clone()).unwrap();
        assert_eq!(configs.configs.len(), 2);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "b");
        assert_eq!(configs.configs[1].id, "2");
        assert_eq!(configs.configs[1].content, "c");

        // Lookup the first entry
        let config4 = configs
            .get_by_id(&DummyConfigInfo {
                id: "1".to_owned(),
                content: "b".to_owned(),
            })
            .unwrap();
        assert_eq!(config4.id, config2.id);
        assert_eq!(config4.content, config2.content);

        // Lookup the second entry
        let config5 = configs
            .get_by_id(&DummyConfigInfo {
                id: "2".to_owned(),
                content: "c".to_owned(),
            })
            .unwrap();
        assert_eq!(config5.id, config3.id);
        assert_eq!(config5.content, config3.content);

        // Test case: can't insert an entry with conflicting content
        let config6 = DummyConfigInfo {
            id: "3".to_owned(),
            content: "c".to_owned(),
        };
        configs.insert_or_update(config6).unwrap_err();
        assert_eq!(configs.configs.len(), 2);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "b");
        assert_eq!(configs.configs[1].id, "2");
        assert_eq!(configs.configs[1].content, "c");
    }

    #[test]
    fn test_remove_config_info() {
        let mut configs = DummyConfigInfos::new();

        let config1 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "a".to_owned(),
        };
        configs.insert_or_update(config1).unwrap();
        let config2 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "b".to_owned(),
        };
        configs.insert_or_update(config2.clone()).unwrap();
        let config3 = DummyConfigInfo {
            id: "2".to_owned(),
            content: "c".to_owned(),
        };
        configs.insert_or_update(config3.clone()).unwrap();
        assert_eq!(configs.configs.len(), 2);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "b");
        assert_eq!(configs.configs[1].id, "2");
        assert_eq!(configs.configs[1].content, "c");

        let config4 = configs
            .remove(&DummyConfigInfo {
                id: "1".to_owned(),
                content: "no value".to_owned(),
            })
            .unwrap();
        assert_eq!(config4.id, config2.id);
        assert_eq!(config4.content, config2.content);
        assert_eq!(configs.configs.len(), 1);
        assert_eq!(configs.configs[0].id, "2");
        assert_eq!(configs.configs[0].content, "c");

        let config5 = configs
            .remove(&DummyConfigInfo {
                id: "2".to_owned(),
                content: "no value".to_owned(),
            })
            .unwrap();
        assert_eq!(config5.id, config3.id);
        assert_eq!(config5.content, config3.content);
        assert_eq!(configs.configs.len(), 0);
    }

    type DummyDeviceInfoList = DeviceConfigInfos<DummyConfigInfo>;

    #[test]
    fn test_insert_or_update_device_info() {
        let mut configs = DummyDeviceInfoList::new();

        let config1 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "a".to_owned(),
        };
        configs.insert_or_update(&config1).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].config.id, "1");
        assert_eq!(configs[0].config.content, "a");

        // Test case: succeed to update an existing entry
        let config2 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "b".to_owned(),
        };
        configs.insert_or_update(&config2 /*  */).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].config.id, "1");
        assert_eq!(configs[0].config.content, "b");

        // Add a second entry
        let config3 = DummyConfigInfo {
            id: "2".to_owned(),
            content: "c".to_owned(),
        };
        configs.insert_or_update(&config3).unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].config.id, "1");
        assert_eq!(configs[0].config.content, "b");
        assert_eq!(configs[1].config.id, "2");
        assert_eq!(configs[1].config.content, "c");

        // Lookup the first entry
        let config4_id = configs
            .get_index_by_id(&DummyConfigInfo {
                id: "1".to_owned(),
                content: "b".to_owned(),
            })
            .unwrap();
        let config4 = &configs[config4_id].config;
        assert_eq!(config4.id, config2.id);
        assert_eq!(config4.content, config2.content);

        // Lookup the second entry
        let config5_id = configs
            .get_index_by_id(&DummyConfigInfo {
                id: "2".to_owned(),
                content: "c".to_owned(),
            })
            .unwrap();
        let config5 = &configs[config5_id].config;
        assert_eq!(config5.id, config3.id);
        assert_eq!(config5.content, config3.content);

        // Test case: can't insert an entry with conflicting content
        let config6 = DummyConfigInfo {
            id: "3".to_owned(),
            content: "c".to_owned(),
        };
        configs.insert_or_update(&config6).unwrap_err();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].config.id, "1");
        assert_eq!(configs[0].config.content, "b");
        assert_eq!(configs[1].config.id, "2");
        assert_eq!(configs[1].config.content, "c");
    }

    #[test]
    fn test_remove_device_info() {
        let mut configs = DummyDeviceInfoList::new();

        let config1 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "a".to_owned(),
        };
        configs.insert_or_update(&config1).unwrap();
        let config2 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "b".to_owned(),
        };
        configs.insert_or_update(&config2).unwrap();
        let config3 = DummyConfigInfo {
            id: "2".to_owned(),
            content: "c".to_owned(),
        };
        configs.insert_or_update(&config3).unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].config.id, "1");
        assert_eq!(configs[0].config.content, "b");
        assert_eq!(configs[1].config.id, "2");
        assert_eq!(configs[1].config.content, "c");

        let config4 = configs.remove(0).unwrap().config;
        assert_eq!(config4.id, config2.id);
        assert_eq!(config4.content, config2.content);
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].config.id, "2");
        assert_eq!(configs[0].config.content, "c");

        let config5 = configs.remove(0).unwrap().config;
        assert_eq!(config5.id, config3.id);
        assert_eq!(config5.content, config3.content);
        assert_eq!(configs.len(), 0);
    }

    #[test]
    fn test_rate_limiter_configs() {
        const SIZE: u64 = 1024 * 1024;
        const ONE_TIME_BURST: u64 = 1024;
        const REFILL_TIME: u64 = 1000;

        let c: TokenBucketConfigInfo = TokenBucketConfigInfo {
            size: SIZE,
            one_time_burst: ONE_TIME_BURST,
            refill_time: REFILL_TIME,
        };
        let b: TokenBucket = c.into();
        assert_eq!(b.capacity(), SIZE);
        assert_eq!(b.one_time_burst(), ONE_TIME_BURST);
        assert_eq!(b.refill_time_ms(), REFILL_TIME);

        let mut rlc = RateLimiterConfigInfo {
            bandwidth: TokenBucketConfigInfo {
                size: SIZE,
                one_time_burst: ONE_TIME_BURST,
                refill_time: REFILL_TIME,
            },
            ops: TokenBucketConfigInfo {
                size: SIZE * 2,
                one_time_burst: 0,
                refill_time: REFILL_TIME * 2,
            },
        };
        let rl: RateLimiter = (&rlc).try_into().unwrap();
        assert_eq!(rl.bandwidth().unwrap().capacity(), SIZE);
        assert_eq!(rl.bandwidth().unwrap().one_time_burst(), ONE_TIME_BURST);
        assert_eq!(rl.bandwidth().unwrap().refill_time_ms(), REFILL_TIME);
        assert_eq!(rl.ops().unwrap().capacity(), SIZE * 2);
        assert_eq!(rl.ops().unwrap().one_time_burst(), 0);
        assert_eq!(rl.ops().unwrap().refill_time_ms(), REFILL_TIME * 2);

        let bandwidth = TokenBucketConfigInfo {
            size: SIZE * 2,
            one_time_burst: ONE_TIME_BURST * 2,
            refill_time: REFILL_TIME * 2,
        };
        rlc.update_bandwidth(bandwidth);
        assert_eq!(rlc.bandwidth.size, SIZE * 2);
        assert_eq!(rlc.bandwidth.one_time_burst, ONE_TIME_BURST * 2);
        assert_eq!(rlc.bandwidth.refill_time, REFILL_TIME * 2);
        assert_eq!(rlc.ops.size, SIZE * 2);
        assert_eq!(rlc.ops.one_time_burst, 0);
        assert_eq!(rlc.ops.refill_time, REFILL_TIME * 2);

        let ops = TokenBucketConfigInfo {
            size: SIZE * 3,
            one_time_burst: ONE_TIME_BURST * 3,
            refill_time: REFILL_TIME * 3,
        };
        rlc.update_ops(ops);
        assert_eq!(rlc.bandwidth.size, SIZE * 2);
        assert_eq!(rlc.bandwidth.one_time_burst, ONE_TIME_BURST * 2);
        assert_eq!(rlc.bandwidth.refill_time, REFILL_TIME * 2);
        assert_eq!(rlc.ops.size, SIZE * 3);
        assert_eq!(rlc.ops.one_time_burst, ONE_TIME_BURST * 3);
        assert_eq!(rlc.ops.refill_time, REFILL_TIME * 3);
    }
}
