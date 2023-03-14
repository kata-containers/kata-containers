extern crate users;
use users::{User, all_users};

extern crate env_logger;


fn main() {
    env_logger::init();

    let mut users: Vec<User> = unsafe { all_users() }.collect();
    users.sort_by(|a, b| a.uid().cmp(&b.uid()));

    for user in users {
        println!("User {} has name {}", user.uid(), user.name().to_string_lossy());
    }
}
