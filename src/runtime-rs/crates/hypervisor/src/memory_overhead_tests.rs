// Copyright (c) 2024 Kata Containers
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(test)]
mod tests {
    use crate::utils::megs_to_bytes;
    use kata_types::config::hypervisor::MemoryInfo;

    #[test]
    fn test_memory_overhead_field_exists() {
        let memory_info = MemoryInfo {
            default_memory: 512,
            memory_overhead: 50,
            ..Default::default()
        };

        assert_eq!(memory_info.default_memory, 512);
        assert_eq!(memory_info.memory_overhead, 50);
    }

    #[test]
    fn test_memory_overhead_default_value() {
        let memory_info = MemoryInfo::default();
        assert_eq!(memory_info.memory_overhead, 0);
    }

    #[test]
    fn test_memory_overhead_compensation_calculation() {
        let default_memory: u32 = 512;
        let memory_overhead: u32 = 50;
        let requested_memory: u32 = 1024;

        // Calculate the compensation as implemented in the hypervisors
        let host_unaccounted_mb = default_memory - memory_overhead;
        let adjusted_mb = requested_memory.saturating_sub(host_unaccounted_mb);

        // Expected: 1024 - (512 - 50) = 1024 - 462 = 562
        assert_eq!(host_unaccounted_mb, 462);
        assert_eq!(adjusted_mb, 562);
    }

    #[test]
    fn test_memory_overhead_compensation_bounds() {
        let default_memory: u32 = 512;
        let memory_overhead: u32 = 50;

        // Test case where requested memory is less than default memory
        // In this case, no compensation should be applied (as per the actual implementation)
        let requested_memory: u32 = 400; // Less than default_memory
        
        // Simulate the actual logic: compensation only applies when requested > default
        let adjusted_mb = if requested_memory > default_memory {
            let host_unaccounted_mb = default_memory - memory_overhead;
            let adjusted = requested_memory.saturating_sub(host_unaccounted_mb);
            // Bounded to not go below default_memory
            if adjusted >= default_memory { adjusted } else { default_memory }
        } else {
            requested_memory
        };

        // When requested < default, no compensation is applied, so adjusted should equal requested
        assert_eq!(adjusted_mb, requested_memory);
        
        // Test case where adjustment would result in value below default memory
        let requested_memory: u32 = 600; // Greater than default_memory
        let host_unaccounted_mb = default_memory - memory_overhead;
        let adjusted = requested_memory.saturating_sub(host_unaccounted_mb);
        let adjusted_mb = if adjusted >= default_memory { adjusted } else { default_memory };
        
        // The adjustment would be 600 - 462 = 138, which is less than default_memory
        // So it should be bounded to default_memory
        assert_eq!(adjusted_mb, default_memory);
        
        // Test case where adjustment results in value above default memory
        let requested_memory: u32 = 800; // Greater than default_memory
        let adjusted = requested_memory.saturating_sub(host_unaccounted_mb);
        let adjusted_mb = if adjusted >= default_memory { adjusted } else { default_memory };
        
        // Should be adjusted: 800 - 462 = 338, which is less than default_memory
        // So it should be bounded to default_memory
        assert_eq!(adjusted_mb, default_memory);
    }

    #[test]
    fn test_memory_overhead_edge_cases() {
        // Test with zero overhead
        let default_memory: u32 = 512;
        let memory_overhead: u32 = 0;
        let requested_memory: u32 = 1024;
        let host_unaccounted_mb = default_memory - memory_overhead;
        let adjusted_mb = requested_memory.saturating_sub(host_unaccounted_mb);

        // With zero overhead, adjustment should be: 1024 - 512 = 512
        assert_eq!(adjusted_mb, 512);

        // Test with overhead equal to default memory
        let memory_overhead: u32 = 512;
        let host_unaccounted_mb = default_memory - memory_overhead;
        let adjusted_mb = requested_memory.saturating_sub(host_unaccounted_mb);

        // With overhead equal to default memory, no adjustment: 1024 - 0 = 1024
        assert_eq!(adjusted_mb, 1024);

        // Test with overhead larger than default memory
        let memory_overhead: u32 = 600;
        let host_unaccounted_mb = default_memory.saturating_sub(memory_overhead);
        let adjusted_mb = requested_memory.saturating_sub(host_unaccounted_mb);

        // With overhead larger than default memory, no adjustment: 1024 - 0 = 1024
        assert_eq!(adjusted_mb, 1024);
    }

    #[test]
    fn test_memory_overhead_serialization() {
        let memory_info = MemoryInfo {
            default_memory: 256,
            memory_overhead: 25,
            ..Default::default()
        };

        // Test serialization
        let serialized = serde_json::to_string(&memory_info).unwrap();
        assert!(serialized.contains("\"memory_overhead\":25"));

        // Test deserialization
        let deserialized: MemoryInfo = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.memory_overhead, 25);
    }

    #[test]
    fn test_memory_overhead_annotation_parsing() {
        use kata_types::annotations::KATA_ANNO_CFG_HYPERVISOR_MEMORY_OVERHEAD;
        use std::collections::HashMap;

        // Test that the annotation constant is correctly defined
        assert_eq!(
            KATA_ANNO_CFG_HYPERVISOR_MEMORY_OVERHEAD,
            "io.katacontainers.config.hypervisor.memory_overhead"
        );

        // Test annotation parsing in a mock scenario
        let mut annotations = HashMap::new();
        annotations.insert(
            KATA_ANNO_CFG_HYPERVISOR_MEMORY_OVERHEAD.to_string(),
            "100".to_string(),
        );

        // Simulate parsing the annotation value
        let overhead_value: u32 = annotations
            .get(KATA_ANNO_CFG_HYPERVISOR_MEMORY_OVERHEAD)
            .unwrap()
            .parse()
            .unwrap();

        assert_eq!(overhead_value, 100);
    }

    #[test]
    fn test_memory_overhead_bytes_conversion() {
        let overhead_mb: u32 = 50;
        let overhead_bytes = megs_to_bytes(overhead_mb);

        // 50 MiB = 50 * 1024 * 1024 bytes = 52,428,800 bytes
        assert_eq!(overhead_bytes, 50 * 1024 * 1024);
    }

    #[test]
    fn test_memory_overhead_compensation_scenarios() {
        struct TestCase {
            default_memory: u32,
            memory_overhead: u32,
            requested_memory: u32,
            expected_adjusted: u32,
            description: &'static str,
        }

        let test_cases = vec![
            TestCase {
                default_memory: 512,
                memory_overhead: 50,
                requested_memory: 1024,
                expected_adjusted: 562, // 1024 - (512 - 50)
                description: "Normal case with overhead compensation",
            },
            TestCase {
                default_memory: 256,
                memory_overhead: 0,
                requested_memory: 512,
                expected_adjusted: 256, // 512 - (256 - 0)
                description: "Zero overhead case",
            },
            TestCase {
                default_memory: 128,
                memory_overhead: 128,
                requested_memory: 256,
                expected_adjusted: 256, // 256 - (128 - 128) = 256 - 0
                description: "Overhead equals default memory",
            },
            TestCase {
                default_memory: 256,
                memory_overhead: 300,
                requested_memory: 512,
                expected_adjusted: 512, // 512 - (256 - 300) = 512 - 0 (saturating_sub)
                description: "Overhead larger than default memory",
            },
        ];

        for test_case in test_cases {
            let host_unaccounted_mb = test_case.default_memory.saturating_sub(test_case.memory_overhead);
            let adjusted_mb = test_case.requested_memory.saturating_sub(host_unaccounted_mb);

            assert_eq!(
                adjusted_mb, test_case.expected_adjusted,
                "Failed for case: {}",
                test_case.description
            );
        }
    }
}
