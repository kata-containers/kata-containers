extern crate users;
use users::{Users, Groups, UsersCache};

extern crate env_logger;


fn main() {
    env_logger::init();

	let cache = UsersCache::new();

	let current_uid = cache.get_current_uid();
	println!("Your UID is {}", current_uid);

	let you = cache.get_user_by_uid(current_uid).expect("No entry for current user!");
	println!("Your username is {}", you.name().to_string_lossy());

	let primary_group = cache.get_group_by_gid(you.primary_group_id()).expect("No entry for your primary group!");
	println!("Your primary group has ID {} and name {}", primary_group.gid(), primary_group.name().to_string_lossy());
}
