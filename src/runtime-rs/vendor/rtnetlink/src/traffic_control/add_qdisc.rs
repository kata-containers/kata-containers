// SPDX-License-Identifier: MIT

use futures::stream::StreamExt;

use crate::{
    packet::{
        tc::{constants::*, nlas},
        NetlinkMessage,
        RtnlMessage,
        TcMessage,
        NLM_F_ACK,
        NLM_F_REQUEST,
        TC_H_MAKE,
    },
    try_nl,
    Error,
    Handle,
};

pub struct QDiscNewRequest {
    handle: Handle,
    message: TcMessage,
    flags: u16,
}

impl QDiscNewRequest {
    pub(crate) fn new(handle: Handle, message: TcMessage, flags: u16) -> Self {
        Self {
            handle,
            message,
            flags: NLM_F_REQUEST | flags,
        }
    }

    /// Execute the request
    pub async fn execute(self) -> Result<(), Error> {
        let Self {
            mut handle,
            message,
            flags,
        } = self;

        let mut req = NetlinkMessage::from(RtnlMessage::NewQueueDiscipline(message));
        req.header.flags = NLM_F_ACK | flags;

        let mut response = handle.request(req)?;
        while let Some(message) = response.next().await {
            try_nl!(message);
        }
        Ok(())
    }

    /// Set handle,
    pub fn handle(mut self, maj: u16, min: u16) -> Self {
        self.message.header.handle = TC_H_MAKE!((maj as u32) << 16, min as u32);
        self
    }

    /// Set parent to root.
    pub fn root(mut self) -> Self {
        assert_eq!(self.message.header.parent, TC_H_UNSPEC);
        self.message.header.parent = TC_H_ROOT;
        self
    }

    /// Set parent
    pub fn parent(mut self, parent: u32) -> Self {
        assert_eq!(self.message.header.parent, TC_H_UNSPEC);
        self.message.header.parent = parent;
        self
    }

    /// New a ingress qdisc
    pub fn ingress(mut self) -> Self {
        assert_eq!(self.message.header.parent, TC_H_UNSPEC);
        self.message.header.parent = TC_H_INGRESS;
        self.message.header.handle = 0xffff0000;
        self.message
            .nlas
            .push(nlas::Nla::Kind("ingress".to_string()));
        self
    }
}

#[cfg(test)]
mod test {
    use std::{fs::File, os::unix::io::AsRawFd, path::Path};

    use futures::stream::TryStreamExt;
    use nix::sched::{setns, CloneFlags};
    use tokio::runtime::Runtime;

    use super::*;
    use crate::{
        new_connection,
        packet::{
            rtnl::tc::nlas::Nla::{HwOffload, Kind},
            LinkMessage,
            AF_UNSPEC,
        },
        NetworkNamespace,
        NETNS_PATH,
        SELF_NS_PATH,
    };

    const TEST_NS: &str = "netlink_test_qdisc_ns";
    const TEST_DUMMY: &str = "test_dummy";

    struct Netns {
        path: String,
        _cur: File,
        last: File,
    }

    impl Netns {
        async fn new(path: &str) -> Self {
            // record current ns
            let last = File::open(Path::new(SELF_NS_PATH)).unwrap();

            // create new ns
            NetworkNamespace::add(path.to_string()).await.unwrap();

            // entry new ns
            let ns_path = Path::new(NETNS_PATH);
            let file = File::open(ns_path.join(path)).unwrap();
            setns(file.as_raw_fd(), CloneFlags::CLONE_NEWNET).unwrap();

            Self {
                path: path.to_string(),
                _cur: file,
                last,
            }
        }
    }
    impl Drop for Netns {
        fn drop(&mut self) {
            println!("exit ns: {}", self.path);
            setns(self.last.as_raw_fd(), CloneFlags::CLONE_NEWNET).unwrap();

            let ns_path = Path::new(NETNS_PATH).join(&self.path);
            nix::mount::umount2(&ns_path, nix::mount::MntFlags::MNT_DETACH).unwrap();
            nix::unistd::unlink(&ns_path).unwrap();
            // _cur File will be closed auto
            // Since there is no async drop, NetworkNamespace::del cannot be called
            // here. Dummy interface will be deleted automatically after netns is
            // deleted.
        }
    }

    async fn setup_env() -> (Handle, LinkMessage, Netns) {
        let netns = Netns::new(TEST_NS).await;

        // Notice: The Handle can only be created after the setns, so that the
        // Handle is the connection within the new ns.
        let (connection, handle, _) = new_connection().unwrap();
        tokio::spawn(connection);
        handle
            .link()
            .add()
            .dummy(TEST_DUMMY.to_string())
            .execute()
            .await
            .unwrap();
        let mut links = handle
            .link()
            .get()
            .match_name(TEST_DUMMY.to_string())
            .execute();
        let link = links.try_next().await.unwrap();
        (handle, link.unwrap(), netns)
    }

    async fn test_async_new_qdisc() {
        let (handle, test_link, _netns) = setup_env().await;
        handle
            .qdisc()
            .add(test_link.header.index as i32)
            .ingress()
            .execute()
            .await
            .unwrap();
        let mut qdiscs_iter = handle
            .qdisc()
            .get()
            .index(test_link.header.index as i32)
            .ingress()
            .execute();

        let mut found = false;
        while let Some(nl_msg) = qdiscs_iter.try_next().await.unwrap() {
            if nl_msg.header.index == test_link.header.index as i32
                && nl_msg.header.handle == 0xffff0000
            {
                assert_eq!(nl_msg.header.family, AF_UNSPEC as u8);
                assert_eq!(nl_msg.header.handle, 0xffff0000);
                assert_eq!(nl_msg.header.parent, TC_H_INGRESS);
                assert_eq!(nl_msg.header.info, 1); // refcount
                assert_eq!(nl_msg.nlas[0], Kind("ingress".to_string()));
                assert_eq!(nl_msg.nlas[2], HwOffload(0));
                found = true;
                break;
            }
        }
        if !found {
            panic!("not found dev:{} qdisc.", test_link.header.index);
        }
    }

    #[test]
    fn test_new_qdisc() {
        Runtime::new().unwrap().block_on(test_async_new_qdisc());
    }
}
