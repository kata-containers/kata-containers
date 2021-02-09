use pin_project::pin_project;

#[repr(packed)] //~ ERROR E0517
enum E1 {
    V(()),
}

#[pin_project]
#[repr(packed)] //~ ERROR E0517
enum E2 {
    V(()),
}

#[repr(packed)] //~ ERROR E0517
#[pin_project]
enum E3 {
    V(()),
}

fn main() {}
