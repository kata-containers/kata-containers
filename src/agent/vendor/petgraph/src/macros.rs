
macro_rules! clone_fields {
    ($name:ident, $($field:ident),+ $(,)*) => (
        fn clone(&self) -> Self {
            $name {
                $(
                    $field : self . $field .clone()
                ),*
            }
        }
    );
}

