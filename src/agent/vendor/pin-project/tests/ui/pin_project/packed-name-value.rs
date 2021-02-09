use pin_project::pin_project;

#[repr(packed = "")] //~ ERROR E0552
struct S1 {
    f: (),
}

#[pin_project]
#[repr(packed = "")] //~ ERROR E0552
struct S2 {
    f: (),
}

#[repr(packed = "")] //~ ERROR E0552
#[pin_project]
struct S3 {
    f: (),
}

fn main() {}
