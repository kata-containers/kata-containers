# scan_fmt ![BuildStatus](https://travis-ci.org/wlentz/scan_fmt.svg?branch=master)
scan_fmt provides a simple scanf()-like input for Rust.  The goal is to make it easier to read data from a string or stdin.

Currently the format string supports the following special sequences:
<pre>
   {{ = escape for '{'
   }} = escape for '}'
   {} = return any value (until next whitespace)
   {d} = return base-10 decimal
   {x} = return hex (0xab or ab)
   {f} = return float
   {*d} = "*" as the first character means "match but don't return"
   {2d} or {2x} or {2f} = limit the maximum width to 2.  Any positive integer works.
   {[...]} = return pattern.
     ^ inverts if it is the first character
     - is for ranges.  For a literal - put it at the start or end.
     To add a literal ] do "[]abc]"
   {e} = doesn't return a value, but matches end of line.  Use this if you
         don't want to ignore potential extra characters at end of input.
   Examples:
     {[0-9ab]} = match 0-9 or a or b
     {[^,.]} = match anything but , or .
   {/.../} = return regex inside of `//`.
     If there is a single capture group inside of the slashes then
     that group will make up the pattern.
   Examples:
     {/[0-9ab]/} = same as {[0-9ab]}, above
     {/a+/} = matches at least one `a`, greedily
     {/jj(a*)jj/} = matches any number of `a`s, but only if
       they're surrounded by two `j`s
</pre>

### Examples
```rust
 #[macro_use] extern crate scan_fmt;
 use std::error::Error ;
 fn main() -> Result<(),Box<dyn Error>> {
   let (a,b,c) = scan_fmt!( "hello 0x12 345 bye",  // input string
                            "hello {x} {} {}",     // format
                            [hex u8], i32, String) ? ;   // type of a-c Options
   assert_eq!( a, 0x12 ) ;
   assert_eq!( b, 345 ) ;
   assert_eq!( c, "bye" ) ;

   println!("Enter something like: 123-22");
   let (c,d) = scanln_fmt!( "{d}-{d}", // format
                            u16, u8) ? ;  // type of a&b Options
   println!("Got {} and {}",c,d) ;
   // Note - currently scanln_fmt! just calls unwrap() on read_line()

   let (a,b) = scan_fmt_some!( "hello 12 345", // input string
                               "hello {} {}",   // format
                               u8, i32) ;   // types
   assert_eq!( a, Some(12) ) ;
   assert_eq!( b, Some(345) ) ;
   Ok(())
  }
```

### Limitations
There is no compile-time warning if the number of {}'s in the format string doesn't match the number of return values.  You'll just get None for extra return values.  See src/lib.rs for more details.
