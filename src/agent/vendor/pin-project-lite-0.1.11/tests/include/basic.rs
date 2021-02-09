// default pin_project! is completely safe.

::pin_project_lite::pin_project! {
    #[derive(Debug)]
    pub struct DefaultStruct<T, U> {
        #[pin]
        pub pinned: T,
        pub unpinned: U,
    }
}
