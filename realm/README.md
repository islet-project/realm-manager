# Realm image

This folder contains the realm image supporting application provisioning. It consists of:
* the realm daemon, called [app-manager](./app-manager),
* [initramfs](./initramfs) of the realm image,
* root CA for checking the signatures of applications in [keys](./keys),
* [linux-rsi](./linux-rsi) implements the RSI calls and exposes them to userland applications,
* [mocked_host](./mocked_host) is a python script that allows to run this setup without entire stack, it simulates the warden daemon.

## Installing build dependencies

    sudo apt-get install autoconf autopoint libtool libsqlite3-dev python3-tomli python3-sphinx ninja-build libglib2.0-dev texinfo libclang-dev libdevmapper-dev

## Building

### Downloading dependencies

    make deps

### Compiling kernel image

    make compile-image

This will create `linux/arch/arm64/Image` which is the kernel with embedded initramfs. You can use it to launch QEMU by utilizing the `-kernel` argument.

### Compiling the kernel and QEMU 

    make compile

### Running QEMU

    make run

This will launch QEMU with the following arguments:

    "tools/qemu/build/qemu-system-aarch64" \
        -machine virt \
        -cpu cortex-a57 \
        -nographic -smp 1 \
        -kernel $(KERNEL_DIR)/arch/arm64/boot/Image \
        -append "console=ttyAMA0" \
        -m 2048

### Starting QEMU with networking support

#### Preparations

Install `virt-manager` to setup networking for QEMU. It can be done manually by calling some iptables, ip and brctl magic but using `virsh` is just way easier.

    sudo apt install virt-manager

Setting up NAT networking for QEMU

    sudo virsh net-define nat100.xml
    sudo virsh net-start nat100
    sudo virsh net-autostart nat100

File __`nat100.xml`__ has the following content:

```
<network>
  <name>nat100</name>
  <forward mode='nat' dev='br0'/>
  <bridge name='virbr100' stp='on' delay='2'/>
  <ip address='192.168.100.1' netmask='255.255.255.0'>
    <dhcp>
      <range start='192.168.100.141' end='192.168.100.254'/>
    </dhcp>
  </ip>
</network>
```

Creating TAP device

    sudo tunctl -t tap100 -u `whoami`
    sudo ip link set tap100 up
    sudo brctl addif virbr100 tap100


#### Launching QEMU

    tools/qemu/build/qemu-system-aarch64 -machine virt \
        -cpu cortex-a57 \
        -nographic -smp 1 \
        -kernel linux/arch/arm64/boot/Image \
        -m 2048 -append "console=ttyAMA0"  \
        -netdev tap,id=mynet0,ifname=tap100,script=no,downscript=no -device e1000,netdev=mynet0,mac=52:55:00:d1:55:01

