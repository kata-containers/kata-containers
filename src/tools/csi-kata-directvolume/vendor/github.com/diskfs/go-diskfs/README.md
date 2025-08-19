# go-diskfs
go-diskfs is a [go](https://golang.org) library for performing manipulation of disks, disk images and filesystems natively in go.

You can do nearly everything that go-diskfs provides using shell tools like gdisk/fdisk/mkfs.vfat/mtools/sgdisk/sfdisk/dd. However, these have the following limitations:

* they need to be installed on your system
* you need to fork/exec to the command (and possibly a shell) to run them
* some are difficult to run without mounting disks, which may not be possible or may be risky in your environment, and almost certainly will require root privileges
* you do not want to launch a VM to run the excellent [libguestfs](https://libguestfs.org) and it may not be installed

go-diskfs performs all modifications _natively_ in go, without mounting any disks.

## Usage
Note: detailed go documentation is available at [godoc.org](https://godoc.org/github.com/diskfs/go-diskfs).

### Concepts
`go-diskfs` has a few basic concepts:

* Disk
* Partition
* Filesystem

#### Disk
A disk represents either a file or block device that you access and manipulate. With access to the disk, you can:

* read, modify or create a partition table
* open an existing or create a new filesystem

#### Partition
A partition is a slice of a disk, beginning at one point and ending at a later one. You can have multiple partitions on a disk, and a partition table that describes how partitions are laid out on the disk.

#### Filesystem
A filesystem is a construct that gives you access to create, read and write directories and files.

You do *not* need a partitioned disk to work with a filesystem; filesystems can be an entire `disk`, just as they can be an entire block device. However, they also can be in a partition in a `disk`

### Working With a Disk
Before you can do anything with a disk - partitions or filesystems - you need to access it.

* If you have an existing disk or image file, you `Open()` it
* If you are creating a new one, usually just disk image files, you `Create()` it

The disk will be opened read-write, with exclusive access. If it cannot do either, it will fail.

Once you have a `Disk`, you can work with partitions or filesystems in it.

#### Partitions on a Disk

The following are the partition actions you can take on a disk:

* `GetPartitionTable()` - if one exists. Will report the table layout and type.
* `Partition()` - partition the disk, overwriting any previous table if it exists

As of this writing, supported partition formats are Master Boot Record (`mbr`) and GUID Partition Table (`gpt`).

#### Filesystems on a Disk
Once you have a valid disk, and optionally partition, you can access filesystems on that disk image or partition.

* `CreateFilesystem()` - create a filesystem in an individual partition or the entire disk
* `GetFilesystem()` - access an existing filesystem in a partition or the entire disk

As of this writing, supported filesystems include `FAT32` and `ISO9660` (a.k.a. `.iso`).

With a filesystem in hand, you can create, access and modify directories and files.

* `Mkdir()` - make a directory in a filesystem
* `Readdir()` - read all of the entries in a directory
* `OpenFile()` - open a file for read, optionally write, create and append

Note that `OpenFile()` is intended to match [os.OpenFile](https://golang.org/pkg/os/#OpenFile) and returns a `godiskfs.File` that closely matches [os.File](https://golang.org/pkg/os/#File)

With a `File` in hand, you then can:

* `Write(p []byte)` to the file
* `Read(b []byte)` from the file
* `Seek(offset int64, whence int)` to set the next read or write to an offset in the file

### Read-Only Filesystems
Some filesystem types are intended to be created once, after which they are read-only, for example `ISO9660`/`.iso` and `squashfs`.

`godiskfs` recognizes read-only filesystems and limits working with them to the following:

* You can `GetFilesystem()` a read-only filesystem and do all read activities, but cannot write to them. Any attempt to `Mkdir()` or `OpenFile()` in write/append/create modes or `Write()` to the file will result in an error.
* You can `CreateFilesystem()` a read-only filesystem and write anything to it that you want. It will do all of its work in a "scratch" area, or temporary "workspace" directory on your local filesystem. When you are ready to complete it, you call `Finalize()`, after which it becomes read-only. If you forget to `Finalize()` it, you get... nothing. The `Finalize()` function exists only on read-only filesystems.

### Example

There are examples in the [examples/](./examples/) directory. Here is one to get you started.

The following example will create a fully bootable EFI disk image. It assumes you have a bootable EFI file (any modern Linux kernel compiled with `CONFIG_EFI_STUB=y` will work) available.

```go
import diskfs "github.com/diskfs/go-diskfs"

espSize int := 100*1024*1024 // 100 MB
diskSize int := espSize + 4*1024*1024 // 104 MB


// create a disk image
diskImg := "/tmp/disk.img"
disk := diskfs.Create(diskImg, diskSize, diskfs.Raw, diskfs.SectorSizeDefault)
// create a partition table
blkSize int := 512
partitionSectors int := espSize / blkSize
partitionStart int := 2048
partitionEnd int := partitionSectors - partitionStart + 1
table := PartitionTable{
	type: partition.GPT,
	partitions:[
		Partition{Start: partitionStart, End: partitionEnd, Type: partition.EFISystemPartition, Name: "EFI System"}
	]
}
// apply the partition table
err = disk.Partition(table)


/*
 * create an ESP partition with some contents
 */
kernel, err := os.ReadFile("/some/kernel/file")

fs, err := disk.CreateFilesystem(0, diskfs.TypeFat32)

// make our directories
err = fs.Mkdir("/EFI/BOOT")
rw, err := fs.OpenFile("/EFI/BOOT/BOOTX64.EFI", os.O_CREATE|os.O_RDRWR)

err = rw.Write(kernel)

```

## Tests
There are two ways to run tests: unit and integration (somewhat loosely defined).

* Unit: these tests run entirely within the go process, primarily test unexported and some exported functions, and may use pre-defined test fixtures in a directory's `testdata/` subdirectory. By default, these are run by running `go test ./...` or just `make unit_test`.
* Integration: these test the exported functions and their ability to create or manipulate correct files. They are validated by running a [docker](https://docker.com) container with the right utilities to validate the output. These are run by running `TEST_IMAGE=diskfs/godiskfs go test ./...` or just `make test`. The value of `TEST_IMAGE` will be the image to use to run tests.

For integration tests to work, the correct docker image must be available. You can create it by running `make image`. Check the [Makefile](./Makefile) to see the `docker build` command used to create it. Running `make test` automatically creates the image for you.

### Integration Test Image
The integration test image contains the various tools necessary to test images: `mtools`, `fdisk`, `gdisk`, etc. It works on precisely one file at a time. In order to avoid docker volume mounting limitations with various OSes, instead of mounting the image `-v`, it expects to receive the image as a `stdin` stream, and saves it internally to the container as `/file.img`.

For example, to test the existence of directory `/abc` on file `$PWD/foo.img`:

```
cat $PWD/foo.img | docker run -i --rm $INT_IMAGE mdir -i /file.img /abc
```


## Plans
Future plans are to add the following:

* embed boot code in `mbr` e.g. `altmbr.bin` (no need for `gpt` since an ESP with `/EFI/BOOT/BOOT<arch>.EFI` will boot)
* `ext4` filesystem
* `Joliet` extensions to `iso9660`
* `Rock Ridge` sparse file support - supports the flag, but not yet reading or writing
* `squashfs` sparse file support - currently treats sparse files as regular files
* `qcow` disk format
