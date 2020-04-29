/*
 * Copyright(c) 2013-2019 Intel Corporation. All rights reserved.
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of version 2 of the GNU General Public License as
 * published by the Free Software Foundation.
 *
 * This program is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * General Public License for more details.
 */

#include <stdio.h>
#include <unistd.h>
#include <stdlib.h>
#include <errno.h>
#include <stdint.h>
#include <fcntl.h>
#include <stdbool.h>

#define __KERNEL__
#include <linux/types.h>
#include <linux/byteorder/little_endian.h>

/*
  Next types, definitions and functions were copied from kernel 4.19.24 source
  code, specifically from nvdimm driver
*/

#define PFN_SIG_LEN 16
#define PFN_SIG "NVDIMM_PFN_INFO"
#define SZ_4K 0x00001000

typedef __u16 u16;
typedef __u8 u8;
typedef __u64 u64;
typedef __u32 u32;

enum nd_pfn_mode {
	PFN_MODE_NONE,
	PFN_MODE_RAM,
	PFN_MODE_PMEM,
};

struct nd_pfn_sb {
	u8 signature[PFN_SIG_LEN];
	u8 uuid[16];
	u8 parent_uuid[16];
	__le32 flags;
	__le16 version_major;
	__le16 version_minor;
	__le64 dataoff; /* relative to namespace_base + start_pad */
	__le64 npfns;
	__le32 mode;
	/* minor-version-1 additions for section alignment */
	__le32 start_pad;
	__le32 end_trunc;
	/* minor-version-2 record the base alignment of the mapping */
	__le32 align;
	u8 padding[4000];
	__le64 checksum;
};

struct nd_gen_sb {
	char reserved[SZ_4K - 8];
	__le64 checksum;
};


u64 nd_fletcher64(void *addr, size_t len, bool le)
{
	u32 *buf = addr;
	u32 lo32 = 0;
	u64 hi32 = 0;
	int i;

	for (i = 0; i < len / sizeof(u32); i++) {
		lo32 += le ? __le32_to_cpu((__le32) buf[i]) : buf[i];
		hi32 += lo32;
	}

	return hi32 << 32 | lo32;
}


/*
 * nd_sb_checksum: compute checksum for a generic info block
 *
 * Returns a fletcher64 checksum of everything in the given info block
 * except the last field (since that's where the checksum lives).
 */
u64 nd_sb_checksum(struct nd_gen_sb *nd_gen_sb)
{
	u64 sum;
	__le64 sum_save;

	sum_save = nd_gen_sb->checksum;
	nd_gen_sb->checksum = 0;
	sum = nd_fletcher64(nd_gen_sb, sizeof(*nd_gen_sb), 1);
	nd_gen_sb->checksum = sum_save;
	return sum;
}


void show_usage(const char* name) {
	printf("Usage: %s IMAGE_FILE  DATA_OFFSET  ALIGNMENT\n", name);
	printf("DATA_OFFSET and ALIGNMENT must be in bytes\n");
}

int main(int argc, char *argv[]) {
	if (argc != 4) {
		show_usage(argv[0]);
		return -1;
	}

	const char* img_path = argv[1];

	char *ptr = NULL;
	const long int data_offset = strtol(argv[2], &ptr, 10);
	if (ptr == argv[2]) {
		fprintf(stderr, "Couldn't convert string '%s' to int\n", argv[2]);
		show_usage(argv[0]);
		return -1;
	}

	ptr = NULL;
	const long int alignment = strtol(argv[3], &ptr, 10);
	if (ptr == argv[3]) {
		fprintf(stderr, "Couldn't convert string '%s' to int\n", argv[3]);
		show_usage(argv[0]);
		return -1;
	}

	printf("Opening file '%s'\n", img_path);
	int fd = open(img_path, O_WRONLY);
	if (fd == -1) {
		perror("open:");
		return -1;
	}

	struct nd_pfn_sb sb = { 0 };

	snprintf((char*)sb.signature, PFN_SIG_LEN, PFN_SIG);
	sb.mode = PFN_MODE_RAM;
	sb.align = alignment;
	sb.dataoff = data_offset;
	sb.version_minor = 2;

	// checksum must be calculated at the end
	sb.checksum = nd_sb_checksum((struct nd_gen_sb*) &sb);

	// NVDIMM driver: SZ_4K is the namespace-relative starting offset
	int ret = lseek(fd, SZ_4K, SEEK_SET);
	if (ret == -1) {
		perror("lseek: ");
		close(fd);
		return -1;
	}

	printf("Writing metadata\n");
	ret = write(fd, &sb, sizeof(sb));
	if (ret == -1) {
		perror("write: ");
	}

	close(fd);
	printf("OK!\n");

	return 0;
}
