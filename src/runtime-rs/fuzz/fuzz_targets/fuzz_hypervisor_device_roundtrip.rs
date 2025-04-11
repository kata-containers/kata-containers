#![no_main]
use hypervisor::device::driver::PciPath;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|path: &str| {
    if let Ok(parsed) = PciPath::convert_from_string(path) {
        let round_trip = parsed.convert_to_string();
        assert_eq!(path, round_trip);
    }
});
