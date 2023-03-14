# ELF Image parsing and loading

The boot process is explained from the following two sides.

## Loader side

It follows ELF standard which is specified in elf.rs.
The entry header and program headers will be interpreted, and PT_LOAD segments
will be loaded into guest memory.

### Where kernel is loaded

There are two ways on deciding where the program segments will be loaded.

- One way is to provide an option and allow vmm to specify where to load the
  image, considering its memory layout.

- The other way is to load image into phdr.p_paddr by default.

## VMM side

### Construct zero page

According to the 64-bit boot protocol, the boot parameters (traditionally known
as "zero page") should be setup, including setup_header, e820 table and other
stuff. However, ELF has no setup_header, nothing returned from ELF loader could
be used to fill boot parameters, vmm is totally responsible for the construction.

### Configure vCPU

- RIP, the start offset of guest memory where kernel is loaded, which is
  returned from loader

- 64 bit mode with paging enabled

- GDT must be configured and loaded

# bzImage

The boot process is also explained from the following two sides.

## Loader side

### What will be returned from loader

bzImage includes two parts, the setup and the compressed kernel. The compressed
kernel part will be loaded into guest memory, and the following three parts
will be returned to the VMM by the loader.

- The start address of loaded kernel

- The offset of memory where kernel is end of loading

- The setup header begin at the offset 0x01f1 of bzImage, this one is an extra
  compared to the return of ELF loader.

### Where kernel is loaded

The same as ELF image loader, there are two ways for deciding where the
compressed kernel will be loaded.

- VMM specify where to load kernel image.

- Load into code32_start (Boot load address) by default.

### Additional checking

As what the boot protocol said, the kernel is a bzImage kernel if the
protocol >= 2.00 and the 0x01 bit(LOAD_HIGH) is the loadflags field is set. Add
this checking to validate the bzImage.

## VMM side

### Construct zero page

While vmm build "zero page" with e820 table and other stuff, bzImage loader will
return the setup header to fill the boot parameters. Meanwhile,
setup_header.init_size is a must to be filled into zero page, which will be used
during head_64.S boot process.

### Configure vCPU

- RIP, the start address of loaded 64-bit kernel returned from loader + 0x200.
  Regarding to the 64-bit boot protocol, kernel is started by jumping to the
  64-bit kernel entry point, which is the start address of loaded 64-bit kernel
  plus 0x200.

- 64 bit mode with paging enabled

- GDT must be configured and loaded


