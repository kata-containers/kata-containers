
use quickcheck::{Arbitrary, Gen, StdGen};
use std::ops::Deref;

#[derive(Copy, Clone, Debug)]
/// quickcheck Arbitrary adaptor - half the size of `T` on average
pub struct Small<T>(pub T);

impl<T> Deref for Small<T> {
    type Target = T;
    fn deref(&self) -> &T { &self.0 }
}

impl<T> Arbitrary for Small<T>
    where T: Arbitrary
{
    fn arbitrary<G: Gen>(g: &mut G) -> Self {
        let sz = g.size() / 2;
        Small(T::arbitrary(&mut StdGen::new(g, sz)))
    }

    fn shrink(&self) -> Box<Iterator<Item=Self>> {
        Box::new((**self).shrink().map(Small))
    }
}

