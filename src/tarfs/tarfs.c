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

#define TARFS_MAGIC (0x54415246535f)

struct tarfs_super {
	u64 inode_table_offset;
	u64 inode_count;
} __packed;

struct tarfs_state {
	struct tarfs_super super;
};

struct tarfs_inode {
	u16 mode;
	u8 padding;
	u8 hmtime; /* High 4 bits of mtime. */
	u32 owner;
	u32 group;
	u32 lmtime; /* Lower 32 bits of mtime. */
	u64 size;
	u64 offset;
}  __packed;

struct tarfs_direntry {
	u64 ino;
	u64 nameoffset;
	u64 namelen;
	u8 type;
	u8 padding[7];
}  __packed;

#define TARFS_BSIZE 512

static struct dentry *tarfs_lookup(struct inode *dir, struct dentry *dentry,
				   unsigned int flags);

static int tarfs_dev_read(struct super_block *sb, unsigned long pos,
			  void *buf, size_t buflen)
{
	/* TODO: Check against the device size here. */
	struct buffer_head *bh;
	unsigned long offset;
	size_t segment;

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
	u64 offset = (u64)inode->i_private;
	int ret = 0;
	char *name_buffer = NULL;
	u64 name_len = 0;
	u64 cur = ctx->pos;
	u64 size = i_size_read(inode);

	/* cur must be aligned to a directory entry. */
	if (ctx->pos % sizeof(struct tarfs_direntry))
		return -ENOENT;

	for (cur = ctx->pos; cur < size; cur += sizeof(disk_dentry)) {
		u64 disk_len;

		/* TODO: Check for overflow in `offset + cur`. */
		ret = tarfs_dev_read(inode->i_sb, offset + cur, &disk_dentry, sizeof(disk_dentry));
		if (ret)
			break;

		disk_len = le64_to_cpu(disk_dentry.namelen);
		if (disk_len > name_len) {
			kfree(name_buffer);
			/* TODO: Check that we don't clamp the allocation here.*/
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

		if (!dir_emit(ctx, name_buffer, disk_len, le64_to_cpu(disk_dentry.ino), disk_dentry.type)) {
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

		pos = (u64)inode->i_private + offset;

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
	struct tarfs_inode disk_inode;
	struct inode *inode;
	const struct tarfs_state *state = sb->s_fs_info;
	int ret;
	u16 mode;

	if (!ino || ino > state->super.inode_count)
		return ERR_PTR(-ENOENT);

	inode = iget_locked(sb, ino);
	if (!inode)
		return ERR_PTR(-ENOMEM);

	if (!(inode->i_state & I_NEW))
		return inode;

	/* TODO: Check that we don't overflow here? */
	ret = tarfs_dev_read(sb, state->super.inode_table_offset + sizeof(struct tarfs_inode) * (ino - 1), &disk_inode, sizeof(disk_inode));
	if (ret < 0)
		goto discard;

	/* TODO: Check that we don't have any extra bits we don't
	 * recognise in mode.
	 */
	mode = le16_to_cpu(disk_inode.mode);
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

	default:
		ret = -ENOENT;
		goto discard;
	}

	set_nlink(inode, 1);

	/* TODO: Initialise these from the disk inode. */
	inode->i_mtime.tv_sec = inode->i_atime.tv_sec = inode->i_ctime.tv_sec = 0;
	inode->i_mtime.tv_nsec = inode->i_atime.tv_nsec = inode->i_ctime.tv_nsec = 0;

	inode->i_mode = mode;
	inode->i_size = le64_to_cpu(disk_inode.size);
	inode->i_blocks = (inode->i_size + TARFS_BSIZE - 1) / TARFS_BSIZE;
	/* TODO: What do we do if we're in a 32-bit machine? */
	inode->i_private = (void *)le64_to_cpu(disk_inode.offset);
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
	u64 offset = (u64)dir->i_private;
	int ret;
	const char *name = dentry->d_name.name;
	size_t len = dentry->d_name.len;
	u64 size = i_size_read(dir);
	u64 cur;

	for (cur = 0; cur < size; cur += sizeof(disk_dentry)) {
		u64 disk_len;

		/* TODO: Ensure we don't overflow here. */
		ret = tarfs_dev_read(dir->i_sb, offset + cur, &disk_dentry, sizeof(disk_dentry));
		if (ret)
			return ERR_PTR(ret);

		disk_len = le64_to_cpu(disk_dentry.namelen);
		/* TODO: Check the name is not too long. */
		if (len != disk_len)
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
	u64 id = huge_encode_dev(sb->s_bdev->bd_dev);

	buf->f_type = TARFS_MAGIC;
	buf->f_namelen = 260; /* TODO: Fix this. */
	buf->f_bsize = TARFS_BSIZE;
	buf->f_bfree = buf->f_bavail = buf->f_ffree;
	buf->f_blocks = bdev_nr_sectors(sb->s_bdev);
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

static int tarfs_fill_super(struct super_block *sb, struct fs_context *fc)
{
	static const struct export_operations tarfs_export_ops = {
		.fh_to_dentry = tarfs_fh_to_dentry,
		.fh_to_parent = tarfs_fh_to_parent,
	};
	static const struct super_operations super_ops = {
		.statfs	= tarfs_statfs,
	};
	struct inode *root;
	sector_t scount;
	struct tarfs_state *state;
	struct buffer_head *bh;
	const struct tarfs_super *super;

	sb_set_blocksize(sb, TARFS_BSIZE);

	sb->s_maxbytes = MAX_LFS_FILESIZE;
	sb->s_magic = TARFS_MAGIC;
	sb->s_flags |= SB_RDONLY | SB_NOATIME;
	sb->s_time_min = 0;
	sb->s_time_max = 0;
	sb->s_op = &super_ops;
	sb->s_export_op = &tarfs_export_ops;

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
	bh = sb_bread(sb, (scount - 1) * SECTOR_SIZE / TARFS_BSIZE);
	if (!bh)
		return -EIO;

	super = (const struct tarfs_super *)bh->b_data;
	state->super.inode_count = le64_to_cpu(super->inode_count);
	state->super.inode_table_offset =
		le64_to_cpu(super->inode_table_offset);

	brelse(bh);

	/* TODO: Validate that offset is within bounds and that adding
	 * all inodes also remains within bounds.
	 */

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

	ret = register_filesystem(&tarfs_fs_type);
	if (ret) {
		pr_err("register_filesystem failed: %d\n", ret);
		return ret;
	}

	return 0;
}

static void __exit tarfs_exit(void)
{
	unregister_filesystem(&tarfs_fs_type);
}

module_init(tarfs_init);
module_exit(tarfs_exit);

MODULE_DESCRIPTION("tarfs");
MODULE_AUTHOR("Wedson Almeida Filho <walmeida@microsoft.com>");
MODULE_LICENSE("GPL");
