## How to use config files

config files must be copied in the kernel source code directory and renamed to `.config`

For example:

```
cp x86_kata_kvm_4.14.x linux-4.14.22/.config
pushd linux-4.14.22
make ARCH=x86_64 -j4
```

## How to modify config files

```
cp x86_kata_kvm_4.14.x linux-4.14.22/.config
pushd linux-4.14.22
make menuconfig
popd
cp linux-4.14.22/.config x86_kata_kvm_4.14.x
```
