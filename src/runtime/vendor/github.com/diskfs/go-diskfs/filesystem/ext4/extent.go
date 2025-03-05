package ext4

import (
	"encoding/binary"
	"fmt"
	"sort"
)

const (
	extentTreeHeaderLength int    = 12
	extentTreeEntryLength  int    = 12
	extentHeaderSignature  uint16 = 0xf30a
	extentTreeMaxDepth     int    = 5
)

// extens a structure holding multiple extents
type extents []extent

// extent a structure with information about a single contiguous run of blocks containing file data
type extent struct {
	// fileBlock block number relative to the file. E.g. if the file is composed of 5 blocks, this could be 0-4
	fileBlock uint32
	// startingBlock the first block on disk that contains the data in this extent. E.g. if the file is made up of data from blocks 100-104 on the disk, this would be 100
	startingBlock uint64
	// count how many contiguous blocks are covered by this extent
	count uint16
}

// equal if 2 extents are equal
//
//nolint:unused // useful function for future
func (e *extent) equal(a *extent) bool {
	if (e == nil && a != nil) || (a == nil && e != nil) {
		return false
	}
	if e == nil && a == nil {
		return true
	}
	return *e == *a
}

// blockCount how many blocks are covered in the extents
//
//nolint:unused // useful function for future
func (e extents) blockCount() uint64 {
	var count uint64
	for _, ext := range e {
		count += uint64(ext.count)
	}
	return count
}

// extentBlockFinder provides a way of finding the blocks on disk that represent the block range of a given file.
// Arguments are the starting and ending blocks in the file. Returns a slice of blocks to read on disk.
// These blocks are in order. For example, if you ask to read file blocks starting at 20 for a count of 25, then you might
// get a single fileToBlocks{block: 100, count: 25} if the file is contiguous on disk. Or you might get
// fileToBlocks{block: 100, count: 10}, fileToBlocks{block: 200, count: 15} if the file is fragmented on disk.
// The slice should be read in order.
type extentBlockFinder interface {
	// findBlocks find the actual blocks for a range in the file, given the start block in the file and how many blocks
	findBlocks(start, count uint64, fs *FileSystem) ([]uint64, error)
	// blocks get all of the blocks for a file, in sequential order, essentially unravels the tree into a slice of extents
	blocks(fs *FileSystem) (extents, error)
	// toBytes convert this extentBlockFinder to bytes to be stored in a block or inode
	toBytes() []byte
	getDepth() uint16
	getMax() uint16
	getBlockSize() uint32
	getFileBlock() uint32
	getCount() uint32
}

var (
	_ extentBlockFinder = &extentInternalNode{}
	_ extentBlockFinder = &extentLeafNode{}
)

// extentNodeHeader represents the header of an extent node
type extentNodeHeader struct {
	depth     uint16 // the depth of tree below here; for leaf nodes, will be 0
	entries   uint16 // number of entries
	max       uint16 // maximum number of entries allowed at this level
	blockSize uint32 // block size for this tree
}

func (e extentNodeHeader) toBytes() []byte {
	b := make([]byte, 12)
	binary.LittleEndian.PutUint16(b[0:2], extentHeaderSignature)
	binary.LittleEndian.PutUint16(b[2:4], e.entries)
	binary.LittleEndian.PutUint16(b[4:6], e.max)
	binary.LittleEndian.PutUint16(b[6:8], e.depth)
	return b
}

// extentChildPtr represents a child pointer in an internal node of extents
// the child could be a leaf node or another internal node. We only would know
// after parsing diskBlock to see its header.
type extentChildPtr struct {
	fileBlock uint32 // extents or children of this cover from file block fileBlock onwards
	count     uint32 // how many blocks are covered by this extent
	diskBlock uint64 // block number where the children live
}

// extentLeafNode represents a leaf node of extents
// it includes the information in the header and the extents (leaf nodes).
// By definition, this is a leaf node, so depth=0
type extentLeafNode struct {
	extentNodeHeader
	extents extents // the actual extents
}

// findBlocks find the actual blocks for a range in the file. leaf nodes already have all of the data inside,
// so the FileSystem reference is unused.
func (e extentLeafNode) findBlocks(start, count uint64, _ *FileSystem) ([]uint64, error) {
	var ret []uint64

	// before anything, figure out which file block is the start and end of the desired range
	end := start + count - 1

	// we are at the bottom of the tree, so we can just return the extents
	for _, ext := range e.extents {
		extentStart := uint64(ext.fileBlock)
		extentEnd := uint64(ext.fileBlock + uint32(ext.count) - 1)

		// Check if the extent does not overlap with the given block range
		if extentEnd < start || extentStart > end {
			continue
		}

		// Calculate the overlapping range
		overlapStart := max(start, extentStart)
		overlapEnd := min(end, extentEnd)

		// Calculate the starting disk block for the overlap
		diskBlockStart := ext.startingBlock + (overlapStart - extentStart)

		// Append the corresponding disk blocks to the result
		for i := uint64(0); i <= overlapEnd-overlapStart; i++ {
			ret = append(ret, diskBlockStart+i)
		}
	}
	return ret, nil
}

// blocks find the actual blocks for a range in the file. leaf nodes already have all of the data inside,
// so the FileSystem reference is unused.
func (e extentLeafNode) blocks(_ *FileSystem) (extents, error) {
	return e.extents, nil
}

// toBytes convert the node to raw bytes to be stored, either in a block or in an inode
func (e extentLeafNode) toBytes() []byte {
	// 12 byte header, 12 bytes per child
	b := make([]byte, 12+12*e.max)
	copy(b[0:12], e.extentNodeHeader.toBytes())

	for i, ext := range e.extents {
		base := (i + 1) * 12
		binary.LittleEndian.PutUint32(b[base:base+4], ext.fileBlock)
		binary.LittleEndian.PutUint16(b[base+4:base+6], ext.count)
		diskBlock := make([]byte, 8)
		binary.LittleEndian.PutUint64(diskBlock, ext.startingBlock)
		copy(b[base+6:base+8], diskBlock[4:6])
		copy(b[base+8:base+12], diskBlock[0:4])
	}
	return b
}

func (e *extentLeafNode) getDepth() uint16 {
	return e.depth
}

func (e *extentLeafNode) getMax() uint16 {
	return e.max
}

func (e *extentLeafNode) getBlockSize() uint32 {
	return e.blockSize
}

func (e *extentLeafNode) getFileBlock() uint32 {
	return e.extents[0].fileBlock
}

func (e *extentLeafNode) getCount() uint32 {
	return uint32(len(e.extents))
}

// extentInternalNode represents an internal node in a tree of extents
// it includes the information in the header and the internal nodes
// By definition, this is an internal node, so depth>0
type extentInternalNode struct {
	extentNodeHeader
	children []*extentChildPtr // the children
}

// findBlocks find the actual blocks for a range in the file. internal nodes need to read the filesystem to
// get the child nodes, so the FileSystem reference is used.
func (e extentInternalNode) findBlocks(start, count uint64, fs *FileSystem) ([]uint64, error) {
	var ret []uint64

	// before anything, figure out which file block is the start and end of the desired range
	end := start + count - 1

	// we are not depth 0, so we have children extent tree nodes. Figure out which ranges we are in.
	// the hard part here is that each child has start but not end or count. You only know it from reading the next one.
	// So if the one we are looking at is in the range, we get it from the children, and keep going
	for _, child := range e.children {
		extentStart := uint64(child.fileBlock)
		extentEnd := uint64(child.fileBlock + child.count - 1)

		// Check if the extent does not overlap with the given block range
		if extentEnd < start || extentStart > end {
			continue
		}

		// read the extent block from the disk
		b, err := fs.readBlock(child.diskBlock)
		if err != nil {
			return nil, err
		}
		ebf, err := parseExtents(b, e.blockSize, uint32(extentStart), uint32(extentEnd))
		if err != nil {
			return nil, err
		}
		blocks, err := ebf.findBlocks(extentStart, uint64(child.count), fs)
		if err != nil {
			return nil, err
		}
		if len(blocks) > 0 {
			ret = append(ret, blocks...)
		}
	}
	return ret, nil
}

// blocks find the actual blocks for a range in the file. leaf nodes already have all of the data inside,
// so the FileSystem reference is unused.
func (e extentInternalNode) blocks(fs *FileSystem) (extents, error) {
	var ret extents

	// we are not depth 0, so we have children extent tree nodes. Walk the tree below us and find all of the blocks
	for _, child := range e.children {
		// read the extent block from the disk
		b, err := fs.readBlock(child.diskBlock)
		if err != nil {
			return nil, err
		}
		ebf, err := parseExtents(b, e.blockSize, child.fileBlock, child.fileBlock+child.count-1)
		if err != nil {
			return nil, err
		}
		blocks, err := ebf.blocks(fs)
		if err != nil {
			return nil, err
		}
		if len(blocks) > 0 {
			ret = append(ret, blocks...)
		}
	}
	return ret, nil
}

// toBytes convert the node to raw bytes to be stored, either in a block or in an inode
func (e extentInternalNode) toBytes() []byte {
	// 12 byte header, 12 bytes per child
	b := make([]byte, 12+12*e.max)
	copy(b[0:12], e.extentNodeHeader.toBytes())

	for i, child := range e.children {
		base := (i + 1) * 12
		binary.LittleEndian.PutUint32(b[base:base+4], child.fileBlock)
		diskBlock := make([]byte, 8)
		binary.LittleEndian.PutUint64(diskBlock, child.diskBlock)
		copy(b[base+4:base+8], diskBlock[0:4])
		copy(b[base+8:base+10], diskBlock[4:6])
	}
	return b
}
func (e *extentInternalNode) getDepth() uint16 {
	return e.depth
}

func (e *extentInternalNode) getMax() uint16 {
	return e.max
}

func (e *extentInternalNode) getBlockSize() uint32 {
	return e.blockSize
}

func (e *extentInternalNode) getFileBlock() uint32 {
	return e.children[0].fileBlock
}

func (e *extentInternalNode) getCount() uint32 {
	return uint32(len(e.children))
}

// parseExtents takes bytes, parses them to find the actual extents or the next blocks down.
// It does not recurse down the tree, as we do not want to do that until we actually are ready
// to read those blocks. This is similar to how ext4 driver in the Linux kernel does it.
// totalBlocks is the total number of blocks covered in this given section of the extent tree.
func parseExtents(b []byte, blocksize, start, count uint32) (extentBlockFinder, error) {
	var ret extentBlockFinder
	// must have at least header and one entry
	minLength := extentTreeHeaderLength + extentTreeEntryLength
	if len(b) < minLength {
		return nil, fmt.Errorf("cannot parse extent tree from %d bytes, minimum required %d", len(b), minLength)
	}
	// check magic signature
	if binary.LittleEndian.Uint16(b[0:2]) != extentHeaderSignature {
		return nil, fmt.Errorf("invalid extent tree signature: %x", b[0x0:0x2])
	}
	e := extentNodeHeader{
		entries:   binary.LittleEndian.Uint16(b[0x2:0x4]),
		max:       binary.LittleEndian.Uint16(b[0x4:0x6]),
		depth:     binary.LittleEndian.Uint16(b[0x6:0x8]),
		blockSize: blocksize,
	}
	// b[0x8:0xc] is used for the generation by Lustre but not standard ext4, so we ignore

	// we have parsed the header, now read either the leaf entries or the intermediate nodes
	switch e.depth {
	case 0:
		leafNode := extentLeafNode{
			extentNodeHeader: e,
		}
		// read the leaves
		for i := 0; i < int(e.entries); i++ {
			start := i*extentTreeEntryLength + extentTreeHeaderLength
			diskBlock := make([]byte, 8)
			copy(diskBlock[0:4], b[start+8:start+12])
			copy(diskBlock[4:6], b[start+6:start+8])
			leafNode.extents = append(leafNode.extents, extent{
				fileBlock:     binary.LittleEndian.Uint32(b[start : start+4]),
				count:         binary.LittleEndian.Uint16(b[start+4 : start+6]),
				startingBlock: binary.LittleEndian.Uint64(diskBlock),
			})
		}
		ret = &leafNode
	default:
		internalNode := extentInternalNode{
			extentNodeHeader: e,
		}
		for i := 0; i < int(e.entries); i++ {
			start := i*extentTreeEntryLength + extentTreeHeaderLength
			diskBlock := make([]byte, 8)
			copy(diskBlock[0:4], b[start+4:start+8])
			copy(diskBlock[4:6], b[start+8:start+10])
			ptr := &extentChildPtr{
				diskBlock: binary.LittleEndian.Uint64(diskBlock),
				fileBlock: binary.LittleEndian.Uint32(b[start : start+4]),
			}
			internalNode.children = append(internalNode.children, ptr)
			if i > 0 {
				internalNode.children[i-1].count = ptr.fileBlock - internalNode.children[i-1].fileBlock
			}
		}
		if len(internalNode.children) > 0 {
			internalNode.children[len(internalNode.children)-1].count = start + count - internalNode.children[len(internalNode.children)-1].fileBlock
		}
		ret = &internalNode
	}

	return ret, nil
}

// extendExtentTree extends extent tree with a slice of new extents
// if the existing tree is nil, create a new one.
// For example, if the input is an extent tree - like the kind found in an inode - and you want to add more extents to it,
// you add the provided extents, and it expands the tree, including creating new internal nodes and writing them to disk, as needed.

func extendExtentTree(existing extentBlockFinder, added *extents, fs *FileSystem, parent *extentInternalNode) (extentBlockFinder, error) {
	// Check if existing is a leaf or internal node
	switch node := existing.(type) {
	case *extentLeafNode:
		return extendLeafNode(node, added, fs, parent)
	case *extentInternalNode:
		return extendInternalNode(node, added, fs, parent)
	case nil:
		// brand new extent tree. The root is in the inode, which has a max of 4 extents.
		return createRootExtentTree(added, fs)
	default:
		return nil, fmt.Errorf("unsupported extentBlockFinder type")
	}
}

func createRootExtentTree(added *extents, fs *FileSystem) (extentBlockFinder, error) {
	// the root always is in the inode, which has a maximum of 4 extents. If it fits within that, we can just create a leaf node.
	if len(*added) <= 4 {
		return &extentLeafNode{
			extentNodeHeader: extentNodeHeader{
				depth:     0,
				entries:   uint16(len(*added)),
				max:       4,
				blockSize: fs.superblock.blockSize,
			},
			extents: *added,
		}, nil
	}
	// in theory, we never should be creating a root internal node. We always should be starting with an extent or two,
	// and later expanding the file.
	// It might be theoretically possible, though, so we will handle it in the future.
	return nil, fmt.Errorf("cannot create root internal node")
}

func extendLeafNode(node *extentLeafNode, added *extents, fs *FileSystem, parent *extentInternalNode) (extentBlockFinder, error) {
	// Check if the leaf node has enough space for the added extents
	if len(node.extents)+len(*added) <= int(node.max) {
		// Simply append the extents if there's enough space
		node.extents = append(node.extents, *added...)
		node.entries = uint16(len(node.extents))

		// Write the updated node back to the disk
		err := writeNodeToDisk(node, fs, parent)
		if err != nil {
			return nil, err
		}

		return node, nil
	}

	// If not enough space, split the node
	newNodes, err := splitLeafNode(node, added, fs, parent)
	if err != nil {
		return nil, err
	}

	// Check if the original node was the root
	if parent == nil {
		// Create a new internal node to reference the split leaf nodes
		var newNodesAsBlockFinder []extentBlockFinder
		for _, n := range newNodes {
			newNodesAsBlockFinder = append(newNodesAsBlockFinder, n)
		}
		newRoot := createInternalNode(newNodesAsBlockFinder, nil, fs)
		return newRoot, nil
	}

	// If the original node was not the root, handle the parent internal node
	parentNode, err := getParentNode(node, fs)
	if err != nil {
		return nil, err
	}

	return extendInternalNode(parentNode, added, fs, parent)
}

func splitLeafNode(node *extentLeafNode, added *extents, fs *FileSystem, parent *extentInternalNode) ([]*extentLeafNode, error) {
	// Combine existing and new extents
	allExtents := node.extents
	allExtents = append(allExtents, *added...)
	// Sort extents by fileBlock to maintain order
	sort.Slice(allExtents, func(i, j int) bool {
		return allExtents[i].fileBlock < allExtents[j].fileBlock
	})

	// Calculate the midpoint to split the extents
	mid := len(allExtents) / 2

	// Create the first new leaf node
	firstLeaf := &extentLeafNode{
		extentNodeHeader: extentNodeHeader{
			depth:     0,
			entries:   uint16(mid),
			max:       node.max,
			blockSize: node.blockSize,
		},
		extents: allExtents[:mid],
	}

	// Create the second new leaf node
	secondLeaf := &extentLeafNode{
		extentNodeHeader: extentNodeHeader{
			depth:     0,
			entries:   uint16(len(allExtents) - mid),
			max:       node.max,
			blockSize: node.blockSize,
		},
		extents: allExtents[mid:],
	}

	// Write new leaf nodes to the disk
	err := writeNodeToDisk(firstLeaf, fs, parent)
	if err != nil {
		return nil, err
	}
	err = writeNodeToDisk(secondLeaf, fs, parent)
	if err != nil {
		return nil, err
	}

	return []*extentLeafNode{firstLeaf, secondLeaf}, nil
}

func createInternalNode(nodes []extentBlockFinder, parent *extentInternalNode, fs *FileSystem) *extentInternalNode {
	internalNode := &extentInternalNode{
		extentNodeHeader: extentNodeHeader{
			depth:     nodes[0].getDepth() + 1, // Depth is 1 more than the children
			entries:   uint16(len(nodes)),
			max:       nodes[0].getMax(), // Assuming uniform max for all nodes
			blockSize: nodes[0].getBlockSize(),
		},
		children: make([]*extentChildPtr, len(nodes)),
	}

	for i, node := range nodes {
		internalNode.children[i] = &extentChildPtr{
			fileBlock: node.getFileBlock(),
			count:     node.getCount(),
			diskBlock: getBlockNumberFromNode(node, parent),
		}
	}

	// Write the new internal node to the disk
	err := writeNodeToDisk(internalNode, fs, parent)
	if err != nil {
		return nil
	}

	return internalNode
}

func getBlockNumberFromNode(node extentBlockFinder, parent *extentInternalNode) uint64 {
	for _, childPtr := range parent.children {
		if childPtrMatchesNode(childPtr, node) {
			return childPtr.diskBlock
		}
	}
	return 0 // Return 0 or an appropriate error value if the block number is not found
}

// Helper function to match a child pointer to a node
func childPtrMatchesNode(childPtr *extentChildPtr, node extentBlockFinder) bool {
	switch n := node.(type) {
	case *extentLeafNode:
		return childPtr.fileBlock == n.extents[0].fileBlock
	case *extentInternalNode:
		// Logic to determine if the childPtr matches the internal node
		// Placeholder: Implement based on your specific matching criteria
		return true
	default:
		return false
	}
}

func extendInternalNode(node *extentInternalNode, added *extents, fs *FileSystem, parent *extentInternalNode) (extentBlockFinder, error) {
	// Find the appropriate child node to extend
	childIndex := findChildNode(node, added)
	childPtr := node.children[childIndex]

	// Load the actual child node from the disk
	childNode, err := loadChildNode(childPtr, fs)
	if err != nil {
		return nil, err
	}

	// Recursively extend the child node
	updatedChild, err := extendExtentTree(childNode, added, fs, node)
	if err != nil {
		return nil, err
	}

	// Update the current internal node to reference the updated child
	switch updatedChild := updatedChild.(type) {
	case *extentLeafNode:
		node.children[childIndex] = &extentChildPtr{
			fileBlock: updatedChild.extents[0].fileBlock,
			count:     uint32(len(updatedChild.extents)),
			diskBlock: getBlockNumberFromNode(updatedChild, node),
		}
	case *extentInternalNode:
		node.children[childIndex] = &extentChildPtr{
			fileBlock: updatedChild.children[0].fileBlock,
			count:     uint32(len(updatedChild.children)),
			diskBlock: getBlockNumberFromNode(updatedChild, node),
		}
	default:
		return nil, fmt.Errorf("unsupported updatedChild type")
	}

	// Check if the internal node is at capacity
	if len(node.children) > int(node.max) {
		// Split the internal node if it's at capacity
		newInternalNodes, err := splitInternalNode(node, node.children[childIndex], fs, parent)
		if err != nil {
			return nil, err
		}

		// Check if the original node was the root
		if parent == nil {
			// Create a new internal node as the new root
			var newNodesAsBlockFinder []extentBlockFinder
			for _, n := range newInternalNodes {
				newNodesAsBlockFinder = append(newNodesAsBlockFinder, n)
			}
			newRoot := createInternalNode(newNodesAsBlockFinder, nil, fs)
			return newRoot, nil
		}

		// If the original node was not the root, handle the parent internal node
		return extendInternalNode(parent, added, fs, parent)
	}

	// Write the updated node back to the disk
	err = writeNodeToDisk(node, fs, parent)
	if err != nil {
		return nil, err
	}

	return node, nil
}

// Helper function to get the parent node of a given internal node
//
//nolint:revive // this parameter will be used eventually
func getParentNode(node extentBlockFinder, fs *FileSystem) (*extentInternalNode, error) {
	// Logic to find and return the parent node of the given node
	// This is a placeholder and needs to be implemented based on your specific tree structure
	return nil, fmt.Errorf("getParentNode not implemented")
}

func splitInternalNode(node *extentInternalNode, newChild *extentChildPtr, fs *FileSystem, parent *extentInternalNode) ([]*extentInternalNode, error) {
	// Combine existing children with the new child
	allChildren := node.children
	allChildren = append(allChildren, newChild)
	// Sort children by fileBlock to maintain order
	sort.Slice(allChildren, func(i, j int) bool {
		return allChildren[i].fileBlock < allChildren[j].fileBlock
	})

	// Calculate the midpoint to split the children
	mid := len(allChildren) / 2

	// Create the first new internal node
	firstInternal := &extentInternalNode{
		extentNodeHeader: extentNodeHeader{
			depth:     node.depth,
			entries:   uint16(mid),
			max:       node.max,
			blockSize: node.blockSize,
		},
		children: allChildren[:mid],
	}

	// Create the second new internal node
	secondInternal := &extentInternalNode{
		extentNodeHeader: extentNodeHeader{
			depth:     node.depth,
			entries:   uint16(len(allChildren) - mid),
			max:       node.max,
			blockSize: node.blockSize,
		},
		children: allChildren[mid:],
	}

	// Write new internal nodes to the disk
	err := writeNodeToDisk(firstInternal, fs, parent)
	if err != nil {
		return nil, err
	}
	err = writeNodeToDisk(secondInternal, fs, parent)
	if err != nil {
		return nil, err
	}

	return []*extentInternalNode{firstInternal, secondInternal}, nil
}

func writeNodeToDisk(node extentBlockFinder, fs *FileSystem, parent *extentInternalNode) error {
	var blockNumber uint64
	if parent != nil {
		blockNumber = getBlockNumberFromNode(node, parent)
	} else {
		blockNumber = getNewBlockNumber(fs)
	}

	if blockNumber == 0 {
		return fmt.Errorf("block number not found for node")
	}

	writableFile, err := fs.backend.Writable()
	if err != nil {
		return err
	}

	data := node.toBytes()
	_, err = writableFile.WriteAt(data, int64(blockNumber)*int64(fs.superblock.blockSize))
	return err
}

// Helper function to get a new block number when there is no parent
//
//nolint:revive // this parameter will be used eventually
func getNewBlockNumber(fs *FileSystem) uint64 {
	// Logic to allocate a new block
	// This is a placeholder and needs to be implemented based on your specific filesystem structure
	return 0 // Placeholder: Replace with actual implementation
}

// Helper function to find the block number of a child node from its parent
func findChildBlockNumber(parent *extentInternalNode, child extentBlockFinder) uint64 {
	for _, childPtr := range parent.children {
		if childPtrMatchesNode(childPtr, child) {
			return childPtr.diskBlock
		}
	}
	return 0
}

func findChildNode(node *extentInternalNode, added *extents) int {
	// Assuming added extents are sorted, find the correct child node to extend
	addedSlice := *added
	for i, child := range node.children {
		if addedSlice[0].fileBlock < child.fileBlock {
			return i - 1
		}
	}
	return len(node.children) - 1
}

// loadChildNode load up a child node from the disk
//
//nolint:unparam // this parameter will be used eventually
func loadChildNode(childPtr *extentChildPtr, fs *FileSystem) (extentBlockFinder, error) {
	data := make([]byte, fs.superblock.blockSize)
	_, err := fs.backend.ReadAt(data, int64(childPtr.diskBlock)*int64(fs.superblock.blockSize))
	if err != nil {
		return nil, err
	}

	// Logic to decode data into an extentBlockFinder (extentLeafNode or extentInternalNode)
	// This is a placeholder and needs to be implemented based on your specific encoding scheme
	var node extentBlockFinder
	// Implement the logic to decode the node from the data
	return node, nil
}
