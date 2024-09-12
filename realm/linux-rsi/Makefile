ROOT = $(shell git rev-parse --show-toplevel)
HERE = ${ROOT}/realm/linux-rsi

export KERNEL_DIR := ${ROOT}/realm/linux
export CROSS_COMPILE := aarch64-none-linux-gnu-
export ARCH := arm64

HEADERS = rsi.h

obj-m += rsi.o

all: module 

module: ${HEADERS}
	make -C ${KERNEL_DIR} M=${HERE} modules

clean:
	make -C ${KERNEL_DIR} M=${HERE} clean
