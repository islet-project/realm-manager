ROOT_DIR             = $(shell git rev-parse --show-toplevel)/realm
CONFIG_DIR           = $(ROOT_DIR)/config
DOWNLOAD_DIR         = $(ROOT_DIR)/.download
TOOLCHAIN_DIR        = $(ROOT_DIR)/toolchain
TOOLS_DIR            = $(ROOT_DIR)/tools

GNU_TOOLCHAIN_URL    = https://developer.arm.com/-/media/Files/downloads/gnu/13.2.rel1/binrel/arm-gnu-toolchain-13.2.rel1-x86_64-aarch64-none-linux-gnu.tar.xz
GNU_TOOLCHAIN_DIR    = $(TOOLCHAIN_DIR)/aarch64-none-linux-gnu/bin

KERNEL_UPSTREAM      = https://git.gitlab.arm.com/linux-arm/linux-cca.git
KERNEL_DIR           = $(ROOT_DIR)/linux
KERNEL_REV           = cca-full/rmm-v1.0-eac5

BUSYBOX_UPSTREAM     = https://github.com/mirror/busybox.git
BUSYBOX_DIR          = $(TOOLS_DIR)/busybox
BUSYBOX_REV          = 1_36_stable

STRACE_UPSTREAM      = https://github.com/strace/strace.git
STRACE_DIR           = $(TOOLS_DIR)/strace

GDB_UPSTREAM         = https://ftp.gnu.org/gnu/gdb/gdb-14.1.tar.xz
GDB_DIR              = $(TOOLS_DIR)/gdb

QEMU_UPSTREAM        = https://github.com/qemu/qemu.git
QEMU_DIR             = $(TOOLS_DIR)/qemu

UTIL_LINUX_UPSTREAM  = https://github.com/util-linux/util-linux.git
UTIL_LINUX_DIR       = $(TOOLS_DIR)/util-linux

RSICTL_UPSTREAM      = https://github.com/islet-project/rsictl.git
RSICTL_DIR           = $(TOOLS_DIR)/rsictl

RSI_KO_DIR           = $(ROOT_DIR)/linux-rsi

APP_MANAGER_DIR      = $(ROOT_DIR)/app-manager

DEVICE_MAPPER_RS_DIR = $(ROOT_DIR)/thirdparty/devicemapper-rs
DEVICE_MAPPER_RS_URL = "https://github.com/stratis-storage/devicemapper-rs.git"

INITRAMFS_DIR        = $(ROOT_DIR)/initramfs
OUT_DIR              = $(ROOT_DIR)/out

QEMU_TAP_DEVICE    ?= tap100
QEMU_VSOCK_CID     ?= 100
EXEC               ?= app-manager

RELEASE            ?= true

GREEN_COLOR = \\033[0;32m
RED_COLOR   = \\033[0;31m
NC          = \\033[0;m

export PATH := $(GNU_TOOLCHAIN_DIR):$(PATH)

makedir:
	@for d in "$(DOWNLOAD_DIR)" "$(TOOLCHAIN_DIR)"; do \
		[ -d "$$d" ] || mkdir -p "$$d"; \
	done


toolchains: makedir
	@echo "$(GREEN_COLOR)Fetching gnu toolchain.$(NC)"
	@[ -f "$(DOWNLOAD_DIR)/aarch64-none-linux-gnu.tar.xz" ] || \
		wget "$(GNU_TOOLCHAIN_URL)" -O "$(DOWNLOAD_DIR)/aarch64-none-linux-gnu.tar.xz"
	@echo "$(GREEN_COLOR)Decompressing.$(NC)"
	@tar xf "$(DOWNLOAD_DIR)/aarch64-none-linux-gnu.tar.xz" -C "$(TOOLCHAIN_DIR)"
	@rm -rf "$(TOOLCHAIN_DIR)/aarch64-none-linux-gnu"
	@mv toolchain/*aarch64-none-linux-gnu* toolchain/aarch64-none-linux-gnu

fetch-linux-kernel:
	@echo "$(GREEN_COLOR)Fetching Linux kernel source.$(NC)"
	@[ -d "$(KERNEL_DIR)" ] || (\
		git clone --depth=1 --branch $(KERNEL_REV) $(KERNEL_UPSTREAM) $(KERNEL_DIR))

fetch-busybox:
	@echo "$(GREEN_COLOR)Fetching busybox source.$(NC)"
	@[ -d "$(BUSYBOX_DIR)" ] || git clone -b $(BUSYBOX_REV) --depth=1 $(BUSYBOX_UPSTREAM) $(BUSYBOX_DIR)

fetch-strace:
	@echo "$(GREEN_COLOR)Fetching strace source.$(NC)"
	@[ -d "$(STRACE_DIR)" ] || git clone --depth=1 $(STRACE_UPSTREAM) $(STRACE_DIR)

fetch-qemu:
	@echo "$(GREEN_COLOR)Fetching QEMU source.$(NC)"
	@[ -d "$(QEMU_DIR)" ] || git clone --depth=1 $(QEMU_UPSTREAM) $(QEMU_DIR)

fetch-gdb:
	@echo "$(GREEN_COLOR)Fetching gdb source.$(NC)"
	@[ -f "$(DOWNLOAD_DIR)/gdb.tar.xz" ] || \
		wget "$(GDB_UPSTREAM)" -O "$(DOWNLOAD_DIR)/gdb.tar.xz"
	@echo "$(GREEN_COLOR)Decompressing.$(NC)"
	@tar xf "$(DOWNLOAD_DIR)/gdb.tar.xz" -C "$(TOOLS_DIR)"
	@rm -rf "$(GDB_DIR)"
	@mv $(TOOLS_DIR)/*gdb* "$(TOOLS_DIR)/gdb"

fetch-util-linux:
	@echo "$(GREEN_COLOR)Fetching util-linux source.$(NC)"
	@[ -d "$(UTIL_LINUX_DIR)" ] || git clone --depth=1 $(UTIL_LINUX_UPSTREAM) $(UTIL_LINUX_DIR)

fetch-rsictl:
	@echo "$(GREEN_COLOR)Fetching rsictl source.$(NC)"
	@[ -d "$(RSICTL_DIR)" ] || git clone --depth=1 $(RSICTL_UPSTREAM) $(RSICTL_DIR)


deps: toolchains fetch-linux-kernel fetch-busybox fetch-strace fetch-gdb fetch-qemu fetch-util-linux fetch-rsictl

compile-busybox: $(BUSYBOX_DIR)
	@echo "$(GREEN_COLOR)Building busybox.$(NC)"
	@cp -v $(CONFIG_DIR)/busybox.config $(BUSYBOX_DIR)/.config
	@ARCH=aarch64 CROSS_COMPILE=aarch64-none-linux-gnu- \
		$(MAKE) -C $(BUSYBOX_DIR) -j $(shell nproc)

compile-strace: $(STRACE_DIR)
	@echo "$(GREEN_COLOR)Building strace.$(NC)"
	@if [ ! -f "$(STRACE_DIR)/Makefile" ]; then \
		cd $(STRACE_DIR) && \
			./bootstrap && \
			./configure --build x86_64-pc-linux-gnu --host aarch64-none-linux-gnu \
				LDFLAGS="-static -pthread" --enable-mpers=check; \
	fi;
	@$(MAKE) -C "$(STRACE_DIR)" -j $(shell proc)

compile-gdbserver: $(GDB_DIR)
	@echo "$(GREEN_COLOR)Building gdbserver.$(NC)"
	@if [ ! -f "$(GDB_DIR)/build/Makefile" ]; then \
		mkdir "$(GDB_DIR)/build"; \
		PATH=$(GNU_TOOLCHAIN_DIR):$$PATH \
			cd "$(GDB_DIR)/build" && \
			$(GDB_DIR)/configure \
					--host="aarch64-none-linux-gnu" \
					--enable-gdbserver \
					--disable-gdb \
					--disable-docs \
					--disable-binutils \
					--disable-gas \
					--disable-sim \
					--disable-gprof \
					--disable-inprocess-agent \
					--prefix="$(GDB_DIR)/bin" \
					CC="aarch64-none-linux-gnu-gcc" \
					CXX="aarch64-none-linux-gnu-g++" \
					LDFLAGS="-static -static-libstdc++"; \
	fi;
	@$(MAKE) -C "$(GDB_DIR)/build" -j $(shell nproc)

compile-util-linux: $(UTIL_LINUX_DIR)
	# We only need blkid
	@echo "$(GREEN_COLOR)Building util-linux (blkid).$(NC)"
	@if [ ! -f "$(UTIL_LINUX_DIR)/Makefile" ]; then \
		cd "$(UTIL_LINUX_DIR)" && \
			./autogen.sh && \
			./configure --host=aarch64-none-linux-gnu --disable-shared --without-tinfo --without-ncurses  --disable-pylibmount; \
	fi;
	@$(MAKE) -C "$(UTIL_LINUX_DIR)" blkid CC=aarch64-none-linux-gnu-gcc LDFLAGS="--static" -j $(shell nproc) && \
		aarch64-none-linux-gnu-strip "$(UTIL_LINUX_DIR)/blkid"

compile-kernel:
	@echo "$(GREEN_COLOR)Building kernel image.$(NC)"
	@if [ ! -f "$(KERNEL_DIR)/.config" ]; then \
		ARCH=arm64 CROSS_COMPILE=aarch64-none-linux-gnu- $(MAKE) -C "$(KERNEL_DIR)" virtconfig; \
		cd "$(KERNEL_DIR)" && "$(KERNEL_DIR)/scripts/kconfig/merge_config.sh" -m "$(KERNEL_DIR)/.config" "$(CONFIG_DIR)/kernel.config"; \
	fi;
	@ARCH=arm64 CROSS_COMPILE=aarch64-none-linux-gnu- $(MAKE) -C "$(KERNEL_DIR)" -j $(shell nproc)

compile-rsi-ko: compile-kernel $(RSI_KO_DIR)
	@echo "$(GREEN_COLOR)Building rsi kernel module.$(NC)"
	@if [ ! -f "$(KERNEL_DIR)/.config" ]; then \
		echo "$(GREEN_COLOR)Precompiling kernel.$(NC)"; \
		ARCH=arm64 CROSS_COMPILE=aarch64-none-linux-gnu- $(MAKE) -C "$(KERNEL_DIR)" virtconfig; \
		cd "$(KERNEL_DIR)" && "$(KERNEL_DIR)/scripts/kconfig/merge_config.sh" -m "$(KERNEL_DIR)/.config" "$(CONFIG_DIR)/kernel.config"; \
		ARCH=arm64 CROSS_COMPILE=aarch64-none-linux-gnu- $(MAKE) -C "$(KERNEL_DIR)" -j $(shell nproc); \
	fi;
	@$(MAKE) -C $(RSI_KO_DIR)

compile-rsictl: $(RSICTL_DIR)
	@echo "$(GREEN_COLOR)Building rsictl.$(NC)"
	@cd "$(RSICTL_DIR)" && cargo build --target=aarch64-unknown-linux-gnu


compile-app-manager:
	@echo "$(GREEN_COLOR)Building app-manager.$(NC)"
ifdef RELEASE
	@cd "$(APP_MANAGER_DIR)" && cargo build --target=aarch64-unknown-linux-gnu --release
else
	@cd "$(APP_MANAGER_DIR)" && cargo build --target=aarch64-unknown-linux-gnu
endif

ifdef RELEASE
prepare-initramfs: compile-busybox  compile-util-linux compile-rsi-ko compile-app-manager
else
prepare-initramfs: compile-busybox  compile-util-linux compile-rsi-ko compile-app-manager compile-strace compile-gdbserver compile-rsictl
endif
	@echo "$(GREEN_COLOR)Preparing initramfs.$(NC)"
	@mkdir -p "$(INITRAMFS_DIR)/usr/bin"
	@mkdir -p "$(INITRAMFS_DIR)/usr/sbin"
	@mkdir -p "$(INITRAMFS_DIR)/lib/modules"
	@cp -v "$(BUSYBOX_DIR)/busybox" "$(INITRAMFS_DIR)/usr/bin"
	@cp -v "$(RSI_KO_DIR)/rsi.ko" "$(INITRAMFS_DIR)/lib/modules/"
	@cp -v "$(UTIL_LINUX_DIR)/blkid" "$(INITRAMFS_DIR)/usr/sbin"
ifdef RELEASE
	@cp -v "$(APP_MANAGER_DIR)/target/aarch64-unknown-linux-gnu/release/app-manager" "$(INITRAMFS_DIR)/usr/bin/"
else
	@cp -v "$(STRACE_DIR)/src/strace" "$(INITRAMFS_DIR)/usr/bin"
	@cp -v "$(GDB_DIR)/build/gdbserver/gdbserver" "$(INITRAMFS_DIR)/usr/bin"
	@cp -v "$(RSICTL_DIR)/target/aarch64-unknown-linux-gnu/debug/rsictl" "$(INITRAMFS_DIR)/usr/bin/"
	@cp -v "$(APP_MANAGER_DIR)/target/aarch64-unknown-linux-gnu/debug/app-manager" "$(INITRAMFS_DIR)/usr/bin/"
endif

compile-image: prepare-initramfs $(KERNEL_DIR)
	@echo "$(GREEN_COLOR)Building image.$(NC)"
	@mkdir -p $(OUT_DIR)
	@cp -v $(KERNEL_DIR)/arch/arm64/boot/Image $(OUT_DIR)/Image
	@cd $(INITRAMFS_DIR) && find . -print0 | \
        cpio --null --create --verbose --format=newc | \
        gzip --best > "$(OUT_DIR)/initramfs.cpio.gz"


compile-qemu: $(QEMU_DIR)
	@echo "$(GREEN_COLOR)Building QEMU.$(NC)"
	@if [ ! -f "$(QEMU_DIR)/build/config-host.mak" ]; then \
		cd "$(QEMU_DIR)" && ./configure --target-list=aarch64-softmmu; \
	fi;
	@$(MAKE) -C "$(QEMU_DIR)" -j $(shell nproc)

compile: compile-image compile-qemu

run: compile
	@echo "$(GREEN_COLOR)Running QEMU.$(NC)"
	"$(QEMU_DIR)/build/qemu-system-aarch64" \
		-machine virt \
		-cpu cortex-a57 \
		-nographic -smp 1 \
		-kernel $(KERNEL_DIR)/arch/arm64/boot/Image \
		-append "console=ttyAMA0" \
		-m 2048

run-only:
	@echo "$(GREEN_COLOR)Running QEMU.$(NC)"
	"$(QEMU_DIR)/build/qemu-system-aarch64" \
		-machine virt \
		-cpu cortex-a57 \
		-nographic -smp 1 \
		-kernel $(KERNEL_DIR)/arch/arm64/boot/Image \
		-append "console=ttyAMA0" \
		-m 2048
