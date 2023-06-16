// SPDX-License-Identifier: GPL-2.0

#include <linux/module.h>
#include <linux/fs.h>
#include <linux/fs_context.h>
#include <linux/pagemap.h>
#include <linux/buffer_head.h>
#include <linux/blkdev.h>
#include <linux/statfs.h>
#include <linux/exportfs.h>
#include <linux/version.h>
#include <linux/xattr.h>

#define TARFS_MAGIC (0x54415246535f)

struct tarfs_super {
	u64 inode_table_offset;
	u64 inode_count;
} __packed;

struct tarfs_state {
	struct tarfs_super super;
	u64 data_size;
};

#define TARFS_INODE_FLAG_OPAQUE 0x1

struct tarfs_inode {
	u16 mode;
	u8 flags;
	u8 hmtime; /* High 4 bits of mtime. */
	u32 owner;
	u32 group;
	u32 lmtime; /* Lower 32 bits of mtime. */
	u64 size;
	u64 offset; /* 64 bits of offset, or 32 LSB are minor dev and 32 MSB are major dev. */
}  __packed;

struct tarfs_direntry {
	u64 ino;
	u64 nameoffset;
	u64 namelen;
	u8 type;
	u8 padding[7];
}  __packed;

struct tarfs_inode_info {
	struct inode inode;
	u64 data_offset;
	u8 flags;
};

#define TARFS_I(ptr) (container_of(ptr, struct tarfs_inode_info, inode))

#define TARFS_BSIZE 4096

static struct kmem_cache *tarfs_inode_cachep;

static struct dentry *tarfs_lookup(struct inode *dir, struct dentry *dentry,
				   unsigned int flags);

static int tarfs_dev_read(struct super_block *sb, u64 pos, void *buf, size_t buflen)
{
	struct buffer_head *bh;
	unsigned long offset;
	size_t segment;
	const struct tarfs_state *state = sb->s_fs_info;

	/* Check for overflows. */
	if (pos + buflen < pos)
		return -ERANGE;

	/* Check that the read range is within the data part of the device. */
	if (pos + buflen > state->data_size)
		return -EIO;

	while (buflen > 0) {
		offset = pos & (TARFS_BSIZE - 1);
		segment = min_t(size_t, buflen, TARFS_BSIZE - offset);
		bh = sb_bread(sb, pos / TARFS_BSIZE);
		if (!bh)
			return -EIO;
		memcpy(buf, bh->b_data + offset, segment);
		brelse(bh);
		buf += segment;
		buflen -= segment;
		pos += segment;
	}

	return 0;
}

static int tarfs_readdir(struct file *file, struct dir_context *ctx)
{
	struct inode *inode = file_inode(file);
	struct tarfs_direntry disk_dentry;
	u64 offset = TARFS_I(inode)->data_offset;
	int ret = 0;
	char *name_buffer = NULL;
	u64 name_len = 0;
	u64 cur = ctx->pos;
	u64 size = i_size_read(inode) / sizeof(disk_dentry) * sizeof(disk_dentry);

	/* cur must be aligned to a directory entry. */
	if (ctx->pos % sizeof(struct tarfs_direntry))
		return -ENOENT;

	/* Make sure we can't overflow the read offset. */
	if (offset + size < offset)
		return -ERANGE;

	/* Make sure the increment of cur won't overflow by limiting size. */
	if (size >= U64_MAX - sizeof(disk_dentry))
		return -ERANGE;

	for (cur = ctx->pos; cur < size; cur += sizeof(disk_dentry)) {
		u64 disk_len;
		u8 type;

		ret = tarfs_dev_read(inode->i_sb, offset + cur, &disk_dentry, sizeof(disk_dentry));
		if (ret)
			break;

		disk_len = le64_to_cpu(disk_dentry.namelen);
		if (disk_len > name_len) {
			kfree(name_buffer);

			if (disk_len > SIZE_MAX)
				return -ENOMEM;

			name_buffer = kmalloc(disk_len, GFP_KERNEL);
			if (!name_buffer)
				return -ENOMEM;
			name_len = disk_len;
		}

		ret = tarfs_dev_read(inode->i_sb,
				le64_to_cpu(disk_dentry.nameoffset),
				name_buffer, disk_len);
		if (ret)
			break;

		/* Filter out bad types. */
		type = disk_dentry.type;
		switch (type) {
		case DT_FIFO:
		case DT_CHR:
		case DT_DIR:
		case DT_BLK:
		case DT_REG:
		case DT_LNK:
		case DT_SOCK:
			break;
		default:
			type = DT_UNKNOWN;
		}

		if (!dir_emit(ctx, name_buffer, disk_len, le64_to_cpu(disk_dentry.ino), type)) {
			kfree(name_buffer);
			return 0;
		}
	}

	kfree(name_buffer);

	if (!ret)
		ctx->pos = cur;

	return ret;
}

static int tarfs_readpage(struct file *file, struct page *page)
{
	struct inode *inode = page->mapping->host;
	loff_t offset, size;
	unsigned long fillsize, pos;
	void *buf;
	int ret;

	buf = kmap_local_page(page);
	if (!buf)
		return -ENOMEM;

	offset = page_offset(page);
	size = i_size_read(inode);
	fillsize = 0;
	ret = 0;
	if (offset < size) {
		size -= offset;
		fillsize = size > PAGE_SIZE ? PAGE_SIZE : size;

		pos = TARFS_I(inode)->data_offset + offset;

		ret = tarfs_dev_read(inode->i_sb, pos, buf, fillsize);
		if (ret < 0) {
			SetPageError(page);
			fillsize = 0;
			ret = -EIO;
		}
	}

	if (fillsize < PAGE_SIZE)
		memset(buf + fillsize, 0, PAGE_SIZE - fillsize);
	if (ret == 0)
		SetPageUptodate(page);

	flush_dcache_page(page);
	kunmap(page);
	unlock_page(page);
	return ret;
}

#if KERNEL_VERSION(5, 19, 0) <= LINUX_VERSION_CODE
static int tarfs_read_folio(struct file *file, struct folio *folio)
{
	return tarfs_readpage(file, &folio->page);
}
#else
static inline void *
alloc_inode_sb(struct super_block *sb, struct kmem_cache *cache, gfp_t gfp)
{
	return kmem_cache_alloc(cache, gfp);
}
#endif

static struct inode *tarfs_iget(struct super_block *sb, u64 ino)
{
	static const struct inode_operations tarfs_symlink_inode_operations = {
		.get_link = page_get_link,
	};
	static const struct inode_operations tarfs_dir_inode_operations = {
		.lookup = tarfs_lookup,
	};
	static const struct file_operations tarfs_dir_operations = {
		.read = generic_read_dir,
		.iterate_shared = tarfs_readdir,
		.llseek = generic_file_llseek,
	};
	static const struct address_space_operations tarfs_aops = {
#if KERNEL_VERSION(5, 19, 0) <= LINUX_VERSION_CODE
		.read_folio = tarfs_read_folio,
#else
		.readpage = tarfs_readpage,
#endif
	};
	struct tarfs_inode_info *info;
	struct tarfs_inode disk_inode;
	struct inode *inode;
	const struct tarfs_state *state = sb->s_fs_info;
	int ret;
	u16 mode;
	u64 offset;

	if (!ino || ino > state->super.inode_count)
		return ERR_PTR(-ENOENT);

	inode = iget_locked(sb, ino);
	if (!inode)
		return ERR_PTR(-ENOMEM);

	if (!(inode->i_state & I_NEW))
		return inode;

	/*
	 * The checks in tarfs_fill_super ensure that we don't overflow while trying to calculate
	 * offset of the inode table entry as long as the inode number is less than inode_count.
	 */
	ret = tarfs_dev_read(sb,
			state->super.inode_table_offset + sizeof(struct tarfs_inode) * (ino - 1),
			&disk_inode, sizeof(disk_inode));
	if (ret < 0)
		goto discard;

	i_uid_write(inode, le32_to_cpu(disk_inode.owner));
	i_gid_write(inode, le32_to_cpu(disk_inode.group));

	offset = le64_to_cpu(disk_inode.offset);
	mode = le16_to_cpu(disk_inode.mode);

	/* Ignore inodes that have unknown mode bits. */
	if (mode & ~(S_IFMT | 0777)) {
		ret = -ENOENT;
		goto discard;
	}

	switch (mode & S_IFMT) {
	case S_IFREG:
		inode->i_fop = &generic_ro_fops;
		inode->i_data.a_ops = &tarfs_aops;
		break;

	case S_IFDIR:
		inode->i_op = &tarfs_dir_inode_operations;
		inode->i_fop = &tarfs_dir_operations;
		break;

	case S_IFLNK:
		inode->i_data.a_ops = &tarfs_aops;
		inode->i_op = &tarfs_symlink_inode_operations;
		inode_nohighmem(inode);
		break;

	case S_IFSOCK:
	case S_IFIFO:
	case S_IFCHR:
	case S_IFBLK:
		init_special_inode(inode, mode, MKDEV(offset >> 32, offset & MINORMASK));
		offset = 0;
		break;

	default:
		ret = -ENOENT;
		goto discard;
	}

	set_nlink(inode, 1);

	inode->i_mtime.tv_sec = inode->i_atime.tv_sec = inode->i_ctime.tv_sec =
		(((u64)disk_inode.hmtime & 0xf) << 32) | le32_to_cpu(disk_inode.lmtime);
	inode->i_mtime.tv_nsec = inode->i_atime.tv_nsec = inode->i_ctime.tv_nsec = 0;

	inode->i_mode = mode;
	inode->i_size = le64_to_cpu(disk_inode.size);
	inode->i_blocks = (inode->i_size + TARFS_BSIZE - 1) / TARFS_BSIZE;

	info = TARFS_I(inode);
	info->data_offset = offset;
	info->flags = disk_inode.flags;

	unlock_new_inode(inode);
	return inode;

discard:
	discard_new_inode(inode);
	return ERR_PTR(ret);
}

static int tarfs_strcmp(struct super_block *sb, unsigned long pos,
			    const char *str, size_t size)
{
	struct buffer_head *bh;
	unsigned long offset;
	size_t segment;
	bool matched;

	/* compare string up to a block at a time. */
	while (size) {
		offset = pos & (TARFS_BSIZE - 1);
		segment = min_t(size_t, size, TARFS_BSIZE - offset);
		bh = sb_bread(sb, pos / TARFS_BSIZE);
		if (!bh)
			return -EIO;
		matched = memcmp(bh->b_data + offset, str, segment) == 0;
		brelse(bh);
		if (!matched)
			return 0;

		size -= segment;
		pos += segment;
		str += segment;
	}

	return 1;
}

static struct dentry *tarfs_lookup(struct inode *dir, struct dentry *dentry,
				   unsigned int flags)
{
	struct inode *inode;
	struct tarfs_direntry disk_dentry;
	u64 offset = TARFS_I(dir)->data_offset;
	int ret;
	const char *name = dentry->d_name.name;
	size_t len = dentry->d_name.len;
	u64 size = i_size_read(dir) / sizeof(disk_dentry) * sizeof(disk_dentry);
	u64 cur;

	/* Make sure we can't overflow the read offset. */
	if (offset + size < offset)
		return ERR_PTR(-ERANGE);

	/* Make sure the increment of cur won't overflow by limiting size. */
	if (size >= U64_MAX - sizeof(disk_dentry))
		return ERR_PTR(-ERANGE);

	for (cur = 0; cur < size; cur += sizeof(disk_dentry)) {
		u64 disk_len;

		ret = tarfs_dev_read(dir->i_sb, offset + cur, &disk_dentry, sizeof(disk_dentry));
		if (ret)
			return ERR_PTR(ret);

		disk_len = le64_to_cpu(disk_dentry.namelen);
		if (len != disk_len || disk_len > SIZE_MAX)
			continue;

		ret = tarfs_strcmp(dir->i_sb, le64_to_cpu(disk_dentry.nameoffset), name, len);
		if (ret < 0)
			return ERR_PTR(ret);

		if (ret == 1) {
			inode = tarfs_iget(dir->i_sb, le64_to_cpu(disk_dentry.ino));
			return d_splice_alias(inode, dentry);
		}
	}

	/* We reached the end of the directory. */
	return ERR_PTR(-ENOENT);
}

static int tarfs_statfs(struct dentry *dentry, struct kstatfs *buf)
{
	struct super_block *sb = dentry->d_sb;
	const struct tarfs_state *state = sb->s_fs_info;
	u64 id = huge_encode_dev(sb->s_bdev->bd_dev);

	buf->f_type = TARFS_MAGIC;
	buf->f_namelen = LONG_MAX;
	buf->f_bsize = TARFS_BSIZE;
	buf->f_bfree = buf->f_bavail = buf->f_ffree = 0;
	buf->f_blocks = state->super.inode_table_offset / TARFS_BSIZE;
	buf->f_files = state->super.inode_count;
	buf->f_fsid = u64_to_fsid(id);
	return 0;
}

static struct inode *tarfs_nfs_get_inode(struct super_block *sb,
		u64 ino, u32 generation)
{
	return tarfs_iget(sb, ino);
}

static struct dentry *tarfs_fh_to_dentry(struct super_block *sb,
		struct fid *fid, int fh_len, int fh_type)
{
	return generic_fh_to_dentry(sb, fid, fh_len, fh_type,
			tarfs_nfs_get_inode);
}

static struct dentry *tarfs_fh_to_parent(struct super_block *sb,
		struct fid *fid, int fh_len, int fh_type)
{
	return generic_fh_to_parent(sb, fid, fh_len, fh_type,
			tarfs_nfs_get_inode);
}

static struct inode *tarfs_alloc_inode(struct super_block *sb)
{
	struct tarfs_inode_info *info;

        info = alloc_inode_sb(sb, tarfs_inode_cachep, GFP_KERNEL);
        if (!info)
                return NULL;

	return &info->inode;
}

static void tarfs_free_inode(struct inode *inode)
{
        kmem_cache_free(tarfs_inode_cachep, TARFS_I(inode));
}

int tarfs_xattr_trusted_get(const struct xattr_handler *handler,
			    struct dentry *unused, struct inode *inode,
			    const char *name, void *buffer, size_t size)
{
	struct tarfs_inode_info *info = TARFS_I(inode);
	bool opaque = (info->flags & TARFS_INODE_FLAG_OPAQUE) != 0;

	if (opaque && strcmp(name, "overlay.opaque") == 0) {
		if (size == 0)
			return 1;
		*(char *)buffer = 'y';
		return 1;
	}

	return -ENODATA;
}

static int tarfs_fill_super(struct super_block *sb, struct fs_context *fc)
{
	static const struct export_operations tarfs_export_ops = {
		.fh_to_dentry = tarfs_fh_to_dentry,
		.fh_to_parent = tarfs_fh_to_parent,
	};
	static const struct super_operations super_ops = {
		.alloc_inode = tarfs_alloc_inode,
		.free_inode = tarfs_free_inode,
		.statfs	= tarfs_statfs,
	};
	static const struct xattr_handler xattr_trusted_handler = {
		.prefix = XATTR_TRUSTED_PREFIX,
		.get = tarfs_xattr_trusted_get,
	};
	static const struct xattr_handler *xattr_handlers[] = {
		&xattr_trusted_handler,
		NULL,
	};
	struct inode *root;
	sector_t scount;
	struct tarfs_state *state;
	struct buffer_head *bh;
	const struct tarfs_super *super;
	u64 inode_table_end;

	sb_set_blocksize(sb, TARFS_BSIZE);

	sb->s_maxbytes = MAX_LFS_FILESIZE;
	sb->s_magic = TARFS_MAGIC;
	sb->s_flags |= SB_RDONLY | SB_NOATIME;
	sb->s_time_min = 0;
	sb->s_time_max = 0;
	sb->s_op = &super_ops;
	sb->s_xattr = xattr_handlers;

	scount = bdev_nr_sectors(sb->s_bdev);
	if (!scount)
		return -ENXIO;

	state = kmalloc(sizeof(*state), GFP_KERNEL);
	if (!state)
		return -ENOMEM;

	/*
	 * state will be freed by kill_sb even if we fail in one of the
	 * functions below.
	 */
	sb->s_fs_info = state;

	/* Read super block then init state. */
	bh = sb_bread(sb, scount * SECTOR_SIZE / TARFS_BSIZE - 1);
	if (!bh)
		return -EIO;

	super = (const struct tarfs_super *)&bh->b_data[TARFS_BSIZE - 512];
	state->super.inode_count = le64_to_cpu(super->inode_count);
	state->super.inode_table_offset =
		le64_to_cpu(super->inode_table_offset);
	state->data_size = scount * SECTOR_SIZE;

	brelse(bh);

	/* This is used to indicate to overlayfs when this superblock limits inodes to 32 bits. */
	if (state->super.inode_count <= U32_MAX)
		sb->s_export_op = &tarfs_export_ops;

	/* Check that the inode table starts within the device data. */
	if (state->super.inode_table_offset >= state->data_size)
		return -E2BIG;

	/* Check that we don't overflow while calculating the offset of the last inode. */
	if (state->super.inode_count > U64_MAX / sizeof(struct tarfs_inode))
		return -ERANGE;

	/* Check that we don't overflow calculating the end of the inode table. */
	inode_table_end = state->super.inode_count * sizeof(struct tarfs_inode) +
		state->super.inode_table_offset;

	if (inode_table_end < state->super.inode_table_offset)
		return -ERANGE;

	/* Check that the inode tanble ends within the device data. */
	if (inode_table_end > state->data_size)
		return -E2BIG;

	root = tarfs_iget(sb, 1);
	if (IS_ERR(root))
		return PTR_ERR(root);

	sb->s_root = d_make_root(root);
	if (!sb->s_root)
		return -ENOMEM;

	return 0;
}

static int tarfs_get_tree(struct fs_context *fc)
{
	int ret;

	ret = get_tree_bdev(fc, tarfs_fill_super);
	if (ret) {
		pr_err("get_tree_bdev failed: %d\n", ret);
		return ret;
	}

	return 0;
}

static int tarfs_reconfigure(struct fs_context *fc)
{
	sync_filesystem(fc->root->d_sb);
	fc->sb_flags |= SB_RDONLY;
	return 0;
}

static int tarfs_init_fs_context(struct fs_context *fc)
{
	static const struct fs_context_operations ops = {
		.get_tree = tarfs_get_tree,
		.reconfigure = tarfs_reconfigure,
	};
	fc->ops = &ops;
	return 0;
}

static void tarfs_kill_sb(struct super_block *sb)
{
	if (sb->s_bdev)
		kill_block_super(sb);
	kfree(sb->s_fs_info);
}

static void tarfs_inode_init_once(void *ptr)
{
	struct tarfs_inode_info *info = ptr;
	inode_init_once(&info->inode);
}

static struct file_system_type tarfs_fs_type = {
	.owner = THIS_MODULE,
	.name = "tar",
	.init_fs_context = tarfs_init_fs_context,
	.kill_sb = tarfs_kill_sb,
	.fs_flags = FS_REQUIRES_DEV,
};
MODULE_ALIAS_FS("tar");

static int __init tarfs_init(void)
{
	int ret;

	tarfs_inode_cachep = kmem_cache_create("tarfs_inode_cache",
			sizeof(struct tarfs_inode_info), 0,
			(SLAB_RECLAIM_ACCOUNT|SLAB_MEM_SPREAD|
			 SLAB_ACCOUNT),
			tarfs_inode_init_once);
	if (!tarfs_inode_cachep) {
		pr_err("kmem_cache_create failed\n");
		return -ENOMEM;
	}

	ret = register_filesystem(&tarfs_fs_type);
	if (ret) {
		pr_err("register_filesystem failed: %d\n", ret);
		kmem_cache_destroy(tarfs_inode_cachep);
		return ret;
	}

	return 0;
}

static void __exit tarfs_exit(void)
{
	unregister_filesystem(&tarfs_fs_type);
	kmem_cache_destroy(tarfs_inode_cachep);
}

module_init(tarfs_init);
module_exit(tarfs_exit);

MODULE_DESCRIPTION("tarfs");
MODULE_AUTHOR("Wedson Almeida Filho <walmeida@microsoft.com>");
MODULE_LICENSE("GPL");
