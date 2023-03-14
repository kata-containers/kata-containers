extern crate users;
use users::{Users, Group, UsersCache, get_user_groups, group_access_list};

extern crate env_logger;


fn main() {
    env_logger::init();

    let cache = UsersCache::new();

    let user = cache.get_user_by_uid(cache.get_current_uid())
        .expect("No current user?");

    let mut groups: Vec<Group> = get_user_groups(user.name(), user.primary_group_id())
        .expect("No user groups?");

    groups.sort_by(|a, b| a.gid().cmp(&b.gid()));
    for group in groups {
        println!("Group {} has name {}", group.gid(), group.name().to_string_lossy());
    }

    let mut groups = group_access_list()
        .expect("Group access list");

    groups.sort_by(|a, b| a.gid().cmp(&b.gid()));
    println!("\nGroup access list:");
    for group in groups {
        println!("Group {} has name {}", group.gid(), group.name().to_string_lossy());
    }
}
