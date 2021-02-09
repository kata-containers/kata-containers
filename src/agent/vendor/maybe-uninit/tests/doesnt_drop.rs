extern crate maybe_uninit;
use maybe_uninit::MaybeUninit;

use std::cell::Cell;

struct DecrementOnDrop<'a>(&'a Cell<usize>);

impl<'a> DecrementOnDrop<'a> {
    pub fn new(ref_:&'a Cell<usize>) -> Self {
        ref_.set(1);
        DecrementOnDrop(ref_)
    }
}

impl<'a> Clone for DecrementOnDrop<'a> {
    fn clone(&self) -> Self {
        self.0.set(self.0.get() + 1);

        DecrementOnDrop(self.0)
    }
}

impl<'a> Drop for DecrementOnDrop<'a>{
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

#[test]
fn doesnt_drop(){
    let count = Cell::new(0);
    let arc = DecrementOnDrop::new(&count);
    let maybe = MaybeUninit::new(arc.clone());
    assert_eq!(count.get(), 2);
    drop(maybe);
    assert_eq!(count.get(), 2);
}
