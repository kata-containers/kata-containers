pub struct BufferPool {
    pool: Vec<Vec<u8>>,
}

impl BufferPool {
    pub fn new(cap: usize) -> BufferPool {
        BufferPool { pool: Vec::with_capacity(cap) }
    }

    pub fn get(&mut self, default_cap: usize) -> Vec<u8> {
        self.pool.pop().unwrap_or_else(|| Vec::with_capacity(default_cap))
    }

    pub fn put(&mut self, mut buf: Vec<u8>) {
        if self.pool.len() < self.pool.capacity(){
            unsafe { buf.set_len(0); }
            self.pool.push(buf);
        }
    }
}
