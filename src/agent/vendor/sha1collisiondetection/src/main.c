/***
* Copyright 2017 Marc Stevens <marc@marc-stevens.nl>, Dan Shumow <danshu@microsoft.com>
* Distributed under the MIT Software License.
* See accompanying file LICENSE.txt or copy at
* https://opensource.org/licenses/MIT
***/

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#ifndef _WIN32
#include <libgen.h>
#endif

#include "sha1.h"

#ifdef _WIN32
char* basename(char* path)
{
    char *base = NULL, *cur = NULL;

    base = path;
    cur = path;
    while (0 != *cur)
    {
        if ('\\' == *cur)
        {
            base = cur + 1;
        }
        cur++;
    }

    return base;
}
#endif

int main(int argc, char** argv)
{
	FILE* fd;
	unsigned char hash2[20];
	char buffer[65536];
	size_t size;
	SHA1_CTX ctx2;
	int i,j,foundcollision;

	if (argc < 2)
	{
		fprintf(stderr, "Usage: %s <file>\n", basename(argv[0]));
		return 1;
	}

	for (i=1; i < argc; ++i)
	{
		SHA1DCInit(&ctx2);

		/* if the program name includes the word 'partial' then also test for reduced-round SHA-1 collisions */
		if (NULL != strstr(argv[0], "partial"))
		{
			SHA1DCSetDetectReducedRoundCollision(&ctx2, 1);
		}

		if(!strcmp(argv[i],"-")) {
			fd = stdin;
		} else {
			fd = fopen(argv[i], "rb");
		}
		if (fd == NULL)
		{
			fprintf(stderr, "cannot open file: %s: %s\n", argv[i], strerror(errno));
			return 1;
		}

		while (1)
		{
			size=fread(buffer,1,65536,fd);
			SHA1DCUpdate(&ctx2, buffer, (unsigned)(size));
			if (size != 65536)
				break;
		}
		if (ferror(fd))
		{
			fprintf(stderr, "error while reading file: %s: %s\n", argv[i], strerror(errno));
			return 1;
		}
		if (!feof(fd))
		{
			fprintf(stderr, "not end of file?: %s: %s\n", argv[i], strerror(errno));
			return 1;
		}

		foundcollision = SHA1DCFinal(hash2,&ctx2);

		for (j = 0; j < 20; ++j)
		{
			sprintf(buffer+(j*2), "%02x", hash2[j]);
		}
		buffer[20*2] = 0;
		if (foundcollision)
		{
			printf("%s *coll* %s\n", buffer, argv[i]);
		}
		else
		{
			printf("%s  %s\n", buffer, argv[i]);
		}

		fclose(fd);
	}
	return 0;
}

#ifdef _MSC_VER
#pragma warning(disable : 4710 )    /* 4710 -- compiler complains about printf,sprintf not being inlined. */
#endif
