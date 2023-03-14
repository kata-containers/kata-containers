# async_once

### async once tool for lazy_static

## Examples
```rust
   use lazy_static::lazy_static;
   use tokio::runtime::Builder;
   use async_once::AsyncOnce;

   lazy_static!{
       static ref FOO : AsyncOnce<u32> = AsyncOnce::new(async{
           1
       });
   }
   let rt = Builder::new_current_thread().build().unwrap();
   rt.block_on(async {
       assert_eq!(FOO.get().await , &1)
   })
```

#### run tests

```bash
   cargo test
   wasm-pack test --headless --chrome --firefox
```


License: MIT OR Apache-2.0
