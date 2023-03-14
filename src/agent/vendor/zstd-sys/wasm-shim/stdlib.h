#include <stddef.h>

#ifndef	_STDLIB_H
#define	_STDLIB_H	1

void *rust_zstd_wasm_shim_malloc(size_t size);
void *rust_zstd_wasm_shim_calloc(size_t nmemb, size_t size);
void rust_zstd_wasm_shim_free(void *ptr);

inline void *malloc(size_t size) {
	return rust_zstd_wasm_shim_malloc(size);
}

inline void *calloc(size_t nmemb, size_t size) {
	return rust_zstd_wasm_shim_calloc(nmemb, size);
}

inline void free(void *ptr) {
	rust_zstd_wasm_shim_free(ptr);
}

#endif // _STDLIB_H
