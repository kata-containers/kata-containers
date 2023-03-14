use std::collections::HashMap;

fn print(name: &str, indent: usize, mods: &HashMap<&str, Vec<&str>>) {
    println!("{}{} {}", if indent == 0 { "-" } else { " " }, " ".repeat(indent), name);

    if let Some(uses_list) = mods.get(name) {
        for name in uses_list {
            print(name, indent + 2, mods);
        }
    }
}

fn main() {
    let modules = procfs::modules().unwrap();

    // each module has a list of what other modules use it.  Let's invert this and create a list of the modules used by each module.
    // This maps a module name to a list of modules that it uses
    let mut map: HashMap<&str, Vec<&str>> = HashMap::new();

    for module in modules.values() {
        for name in &module.used_by {
            map.entry(name).or_default().push(&module.name);
        }
    }

    // println!("{:?}", map["xt_policy"]);
    for modname in map.keys() {
        print(modname, 0, &map);
    }
}
