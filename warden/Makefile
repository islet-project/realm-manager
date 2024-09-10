ROOT_DIR             = $(shell git rev-parse --show-toplevel)/warden
BIN_DIR	             = $(ROOT_DIR)/bin
DNSMASQ_UPSTREAM	 = https://github.com/imp/dnsmasq
DNSMASQ_DIR			 = $(BIN_DIR)/.dnsmasq

CC					 = $(shell git rev-parse --show-toplevel)/realm/toolchain/aarch64-none-linux-gnu/bin/aarch64-none-linux-gnu-gcc

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
