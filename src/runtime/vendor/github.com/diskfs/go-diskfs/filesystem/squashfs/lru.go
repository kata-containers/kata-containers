package squashfs

import (
	"sync"
)

// A simple least recently used cache
type lru struct {
	mu        sync.Mutex
	cache     map[int64]*lruBlock // cache keyed on block position in file
	maxBlocks int                 // max number of blocks in cache
	root      lruBlock            // root block in LRU circular list
}

// A data block to store in the lru cache
type lruBlock struct {
	mu   sync.Mutex // lock while fetching
	data []byte     // data block - nil while being fetched
	prev *lruBlock  // prev block in LRU list
	next *lruBlock  // next block in LRU list
	pos  int64      // position it was read off disk
	size uint16     // compressed size on disk
}

// Create a new LRU cache of a maximum of maxBlocks blocks of size
func newLRU(maxBlocks int) *lru {
	l := &lru{
		cache:     make(map[int64]*lruBlock),
		maxBlocks: maxBlocks,
		root: lruBlock{
			pos: -1,
		},
	}
	l.root.prev = &l.root // circularly link the root node
	l.root.next = &l.root
	return l
}

// Unlink the block from the list
func (l *lru) unlink(block *lruBlock) {
	block.prev.next = block.next
	block.next.prev = block.prev
	block.prev = nil
	block.next = nil
}

// Pop a block from the end of the list
func (l *lru) pop() *lruBlock {
	block := l.root.prev
	if block == &l.root {
		panic("internal error: list empty")
	}
	l.unlink(block)
	return block
}

// Add a block to the start of the list
func (l *lru) push(block *lruBlock) {
	oldHead := l.root.next
	l.root.next = block
	block.prev = &l.root
	block.next = oldHead
	oldHead.prev = block
}

// ensure there are no more than n blocks in the cache
func (l *lru) trim(maxBlocks int) {
	for len(l.cache) > maxBlocks && len(l.cache) > 0 {
		// Remove a block from the cache
		block := l.pop()
		delete(l.cache, block.pos)
	}
}

// add block to the cache, pruning the cache as appropriate
func (l *lru) add(block *lruBlock) {
	l.trim(l.maxBlocks - 1)
	l.cache[block.pos] = block
	l.push(block)
}

// Fetch data returning size used from input and error
//
// data should be a subslice of buf
type fetchFn func() (data []byte, size uint16, err error)

// Get the block at pos from the cache.
//
// If it isn't found in the cache then fetch() is called to get it.
//
// This does read through caching and takes care not to block parallel
// calls to the fetch() function.
func (l *lru) get(pos int64, fetch fetchFn) (data []byte, size uint16, err error) {
	if l == nil {
		return fetch()
	}
	l.mu.Lock()
	block, found := l.cache[pos]
	if !found {
		// Add an empty block with data == nil
		block = &lruBlock{
			pos: pos,
		}
		// Add it to the cache and the tail of the list
		l.add(block)
	} else {
		// Remove the block from the list
		l.unlink(block)
		// Add it back to the start
		l.push(block)
	}
	block.mu.Lock() // transfer the lock to the block
	l.mu.Unlock()
	defer block.mu.Unlock()

	if block.data != nil {
		return block.data, block.size, nil
	}

	// Fetch the block
	data, size, err = fetch()
	if err != nil {
		return nil, 0, err
	}
	block.data = data
	block.size = size
	return data, size, nil
}

// Sets the number of blocks to be used in the cache
//
// It makes sure that there are no more than maxBlocks in the cache.
func (l *lru) setMaxBlocks(maxBlocks int) {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.maxBlocks = maxBlocks
	l.trim(l.maxBlocks)
}
