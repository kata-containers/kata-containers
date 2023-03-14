use caps::runtime;

fn main() {
    let amb_set = runtime::ambient_set_supported().is_ok();
    println!("Ambient set supported: {}", amb_set);

    let all = caps::all();
    let supported = runtime::thread_all_supported();
    let missing = all.difference(&supported);
    println!("Unsupported new capabilities: {:?}", missing);
}
