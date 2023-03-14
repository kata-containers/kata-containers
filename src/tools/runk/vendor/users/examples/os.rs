extern crate users;
use users::{Users, Groups, UsersCache};
use users::os::unix::{UserExt, GroupExt};
//use users::os::bsd::UserExt as BSDUserExt;

extern crate env_logger;


fn main() {
    env_logger::init();

	let cache = UsersCache::new();

	let current_uid = cache.get_current_uid();
	println!("Your UID is {}", current_uid);

	let you = cache.get_user_by_uid(current_uid).expect("No entry for current user!");
	println!("Your username is {}", you.name().to_string_lossy());
	println!("Your shell is {}", you.shell().display());
	println!("Your home directory is {}", you.home_dir().display());

    // The two fields below are only available on BSD systems.
    // Linux systems don’t have the fields in their `passwd` structs!
    //println!("Your password change timestamp is {}", you.password_change_time());
    //println!("Your password expiry timestamp is {}", you.password_expire_time());

	let primary_group = cache.get_group_by_gid(you.primary_group_id()).expect("No entry for your primary group!");
	println!("Your primary group has ID {} and name {}", primary_group.gid(), primary_group.name().to_string_lossy());

	if primary_group.members().is_empty() {
		println!("There are no other members of that group.");
	}
	else {
		for username in primary_group.members() {
			println!("User {} is also a member of that group.", username.to_string_lossy());
		}
	}
}
