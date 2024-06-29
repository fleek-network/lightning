# Lightning eBPF Applications

## Crates

- `guard` - Contains the control application for our eBPF-based security system. 
- `ebpf` - Contains our eBPF kernel-space programs.

## Running

You can run the eBPF control application process via the Lightning Node CLI.

## Requirements

You need a Linux kernel with version ≥ 5.8 and:

- BTF support
- BPF LSM support

You can check if your kernel has BTF support by checking whether file `/sys/kernel/btf/vmlinux` exists.

You also need support for BPF LSM. Run the command below and if the output is `CONFIG_BPF_LSM=y`, you have support for it.

`$ cat /boot/config-$(uname -r) | grep BPF_LSM`

`CONFIG_BPF_LSM=y`

Run the command below to check if the `bpf` option is enabled for LSM.

`$ cat /sys/kernel/security/lsm`

`ndlock,lockdown,yama,integrity,apparmor`

The output above does not include the bpf option so you will need to modify your grub 
configuration by adding `bpf` to `GRUB_CMDLINE_LINUX` in  `/etc/default/grub` as shown below.

`GRUB_CMDLINE_LINUX="lsm=ndlock,lockdown,yama,integrity,apparmor,bpf"`

Update your grub then restart your system and check eBPF ``/sys/kernel/security/lsm`` again to make sure `bpf` is included.

## Dependencies

> You need a Linux platform to run our eBPF solution.

1. Install Rust toolchains.

```
rustup install stable
rustup toolchain install nightly --component rust-src
```

2. Install `bpf-linker` via the following command if running on a Linux x86_64 system.
If you're on any other architecture, please see the `aya` [docs](https://aya-rs.dev/book/start/development/) on how to install it.

```
cargo install bpf-linker
```

3. Install the `bindgen` executable.

```
cargo install bindgen-cli
```
