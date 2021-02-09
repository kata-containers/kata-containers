/// A basic example of /proc/pressure/ usage.

fn main() {
    println!("memory pressure: {:#?}", procfs::MemoryPressure::new());
    println!("cpu pressure: {:#?}", procfs::CpuPressure::new());
    println!("io pressure: {:#?}", procfs::IoPressure::new());
}
