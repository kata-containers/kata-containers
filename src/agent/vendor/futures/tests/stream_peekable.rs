#[test]
fn peekable() {
    use futures::executor::block_on;
    use futures::pin_mut;
    use futures::stream::{self, Peekable, StreamExt};

    block_on(async {
        let peekable: Peekable<_> = stream::iter(vec![1u8, 2, 3]).peekable();
        pin_mut!(peekable);
        assert_eq!(peekable.as_mut().peek().await, Some(&1u8));
        assert_eq!(peekable.collect::<Vec<u8>>().await, vec![1, 2, 3]);
    });
}
