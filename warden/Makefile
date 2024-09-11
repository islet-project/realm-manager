REALM_DIR			= $(shell git rev-parse --show-toplevel)/realm
ROOT_DIR           	= $(shell git rev-parse --show-toplevel)/warden
BIN_DIR	           	= $(ROOT_DIR)/bin
DNSMASQ_UPSTREAM	= https://github.com/imp/dnsmasq
DNSMASQ_DIR			= $(BIN_DIR)/.dnsmasq

export TEST_DIR	     			  		= $(ROOT_DIR)/test
export TEST_BIN_DIR  			  		= $(TEST_DIR)/debug/deps
export TAP_DEVICE	 			  		= tap2000
export RUST_TEST_TIME_INTEGRATION		= 240
export RUST_LOG							= info
export REALM_QEMU_PATH					= $(REALM_DIR)/tools/qemu/build/qemu-system-aarch64
export DHCP_EXEC_PATH					= /usr/sbin/dnsmasq
export NAT_NETWORK_NAME					= virtbWarden
export REALM_KERNEL_PATH				= $(REALM_DIR)/linux/arch/arm64/boot/Image

CC	= $(REALM_DIR)/toolchain/aarch64-none-linux-gnu/bin/aarch64-none-linux-gnu-gcc

GREEN_COLOR = \\033[0;32m
RED_COLOR   = \\033[0;31m
NC          = \\033[0;m

all: dnsmasq
	@echo "$(GREEN_COLOR)Compile warden_daemon and client for ARM.$(NC)"
	@RUSTFLAGS='-C target-feature=+crt-static' cargo build --target=aarch64-unknown-linux-gnu --config target.aarch64-unknown-linux-gnu.linker=\"aarch64-linux-gnu-gcc\" -r --out-dir $(BIN_DIR) -Z unstable-options

dnsmasq: fetch-dnsmasq compile-dnsmasq cleanup

fetch-dnsmasq:
	@echo "$(GREEN_COLOR)Fetching Dnsmasq sources.$(NC)"
	@[ -d "$(DNSMASQ_DIR)" ] || git clone --depth=1 $(DNSMASQ_UPSTREAM) $(DNSMASQ_DIR)

compile-dnsmasq:
	@echo "$(GREEN_COLOR)Building dnsmasq.$(NC)"
	@$(MAKE) -C $(DNSMASQ_DIR) LDFLAGS="-static -static-libgcc -static-libstdc++" CC=$(CC) -j $(shell proc)

cleanup:
	@echo "$(GREEN_COLOR)Cleaning dnsmasq.$(NC)"
	@cp -f $(DNSMASQ_DIR)/src/dnsmasq $(BIN_DIR)
	@rm -rf $(DNSMASQ_DIR)

test: test-compile test-run test-cleanup

test-compile:
	@cargo test --target-dir $(TEST_DIR)
	@cargo test --no-run --target-dir $(TEST_DIR)

test-run:
	@for d in $(shell find $(TEST_BIN_DIR) -executable -name '*_test-*'); do \
		sudo TAP_DEVICE=$(TAP_DEVICE) RUST_TEST_TIME_INTEGRATION=$(RUST_TEST_TIME_INTEGRATION) RUST_LOG=$(RUST_LOG) REALM_QEMU_PATH=$(REALM_QEMU_PATH) DHCP_EXEC_PATH=$(DHCP_EXEC_PATH) NAT_NETWORK_NAME=$(NAT_NETWORK_NAME) REALM_KERNEL_PATH=$(REALM_KERNEL_PATH) $$d  --ignored --nocapture; \
	done

test-cleanup:
	@rm -rf $(TEST_DIR)