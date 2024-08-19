#!/usr/bin/env python3

import argparse
import uuid
import shlex
import json
import sys
import subprocess
import os
import socket
import struct
from typing import List
from dataclasses import dataclass
from gpt_image.disk import Disk
from gpt_image.partition import Partition, PartitionType

APP_ID = uuid.UUID("5d63a211-e8aa-4179-ac22-af7e843a3f43")
IMAGE_UUID = "0178460a-ffea-4674-8e17-8530758c4c2e"
DATA_UUID = "259c493c-e5d2-4fd4-a962-7efd45a0bd91"

@dataclass()
class App:
    id: str
    name: str
    version: str
    image_registry: str
    image_part_uuid: str
    data_part_uuid: str

class MockedWarden():
    def __init__(self, kernel, vsock_port = 1337, guest_cid = 1227, tap_device = "tap100", qemu_serial = "tcp:localhost:1337"):
        self.vsock_port = vsock_port
        self.guest_cid = guest_cid
        self.tap_device = tap_device
        self.qemu_serial = qemu_serial
        self.sock = socket.socket(socket.AF_VSOCK, socket.SOCK_STREAM, 0)
        self.sock.bind((socket.VMADDR_CID_ANY, vsock_port))
        self.sock.listen(1)
        self.kernel = kernel

    def prepare_disk(self):
        if not os.path.isfile("disk.raw"):
            disk = Disk("disk.raw")
            disk.create(1024 * 1024 * 1024)

            image_part = Partition("image", 256 * 1024 * 1024, PartitionType.LINUX_FILE_SYSTEM.value, partition_guid=IMAGE_UUID)
            data_part = Partition("data", 256 * 1024 * 1024, PartitionType.LINUX_FILE_SYSTEM.value, partition_guid=DATA_UUID)

            disk.table.partitions.add(image_part)
            disk.table.partitions.add(data_part)
            disk.commit()

    def run_qemu(self):
        self.qemu = subprocess.Popen(shlex.split(f"""
 "../tools/qemu/build/qemu-system-aarch64" \
        -machine virt \
        -cpu cortex-a57 \
        -nographic -smp 1 \
        -kernel {self.kernel} \
        -append "console=ttyAMA0" \
        -m 2048 -drive file=disk.raw  -netdev tap,id=mynet0,ifname={self.tap_device},script=no,downscript=no -device e1000,netdev=mynet0,mac=52:55:00:d1:55:01 \
        -device vhost-vsock-pci,id=vhost-vsock-pci0,guest-cid={self.guest_cid} \
        -serial {self.qemu_serial}
                                                 """), stdin=-1, stdout=-1)

    def start(self):
        self.prepare_disk()
        self.run_qemu()
        self.reconnect()

    def transaction(self, req, read_resp = True):
        s = json.dumps(req)
        l = struct.pack(">I", len(s))
        data = l + s.encode()

        self.conn.sendall(data)

        if read_resp:
            l = self.conn.recv(4)
            l = struct.unpack(">I", l)[0]
            resp = self.conn.recv(l)
            return json.loads(resp.decode())

    def provision(self, apps: List[App]):
        o = {"ProvisionInfo": [i.__dict__ for i in apps]}
        return self.transaction(o)

    def start_app(self, id=APP_ID):
        o = {"StartApp": str(id)}
        return self.transaction(o)

    def stop_app(self, id=APP_ID):
        o = {"StopApp": str(id)}
        return self.transaction(o)

    def kill_app(self, id=APP_ID):
        o = {"KillApp": str(id)}
        return self.transaction(o)

    def check_app(self, id=APP_ID):
        o = {"CheckStatus": str(id)}
        return self.transaction(o)

    def shutdown(self):
        o = {"Shutdown": []}
        self.transaction(o, read_resp=False)
        self.qemu.communicate()

    def reboot(self):
        o = {"Reboot": []}
        self.transaction(o, read_resp=False)

    def invalid_json(self):
        o = {"jfdewhfdiwuehgfui": "dwehudwqiuydfg23quy"}
        return self.transaction(o)

    def send_exmaple_provision_info(self):
        r = self.provision([
            App(id=str(APP_ID), name="Test app", version="1.0", image_registry="http://registry.com", image_part_uuid=str(IMAGE_UUID), data_part_uuid=str(DATA_UUID))
        ])
        return r

    def reconnect(self):
        print(f"Waiting for connection from realm")
        self.conn, self.addr = self.sock.accept()
        print(f"Accepted connection from {self.addr}")

    def wait_for_qemu(self):
        return self.qemu.communicate()


def main():
    main_parser = argparse.ArgumentParser()
    main_parser.add_argument("--vsock_port", type=int, default=1337)
    main_parser.add_argument("--guest-cid", type=int, default=1337)
    main_parser.add_argument("--tap-device", type=str, default="tap100")
    main_parser.add_argument("--qemu-serial", type=str, default="tcp:localhost:1337")
    main_parser.add_argument('--kernel', type=str, default='../linux/arch/arm64/boot/Image')
    main_parser.add_argument("--test", action='store_true', default=False)
    args = main_parser.parse_args()


    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest='command')

    start_app_parser = subparsers.add_parser('start_app')
    start_app_parser.add_argument('--uuid', type=uuid.UUID, default=APP_ID)

    stop_app_parser = subparsers.add_parser('stop_app')
    stop_app_parser.add_argument('--uuid', type=uuid.UUID, default=APP_ID)

    kill_app_parser = subparsers.add_parser('kill_app')
    kill_app_parser.add_argument('--uuid', type=uuid.UUID, default=APP_ID)

    check_app_parser = subparsers.add_parser('check_app')
    check_app_parser.add_argument('--uuid', type=uuid.UUID, default=APP_ID)

    _ = subparsers.add_parser("reboot")
    _ = subparsers.add_parser("shutdown")
    _ = subparsers.add_parser("launch", help="Launch QEMU after issued shutdown")
    _ = subparsers.add_parser("invalid_json")
    _ = subparsers.add_parser("exit")

    host = MockedWarden(kernel=args.kernel, vsock_port=args.vsock_port, guest_cid=args.guest_cid, tap_device=args.tap_device, qemu_serial=args.qemu_serial)
    host.start()
    r = host.send_exmaple_provision_info()

    if args.test:
        assert r == {'Success': []}

        r = host.stop_app()
        assert r == {'Success': []}

        r = host.check_app()
        assert r == {'ApplicationNotStarted': []}

        r = host.start_app()
        assert r == {'Success': []}

        r = host.check_app()
        assert r == {'ApplicationIsRunning': []}

        r = host.kill_app()
        assert r == {'Success': []}

        r = host.check_app()
        assert r == {'ApplicationNotStarted': []}

        host.reboot()
        host.reconnect()
        r = host.send_exmaple_provision_info()
        assert r == {'Success': []}

        r = host.check_app()
        assert r == {'ApplicationIsRunning': []}

        host.shutdown()
        host.wait_for_qemu()
        print("Test pass")

    else:
        print(f"Provisioning finished with {r}")

        while True:
            sys.stdout.write("> ")
            sys.stdout.flush()
            line = sys.stdin.readline()
            try:
                args = parser.parse_args(shlex.split(line.strip()))
            except:
                continue

            if "command" in args:
                cmd = args.command

                r = None
                if cmd == "start_app":
                    r = host.start_app(id=args.uuid)
                elif cmd == "stop_app":
                    r = host.stop_app(id=args.uuid)
                elif cmd == "kill_app":
                    r = host.kill_app(id=args.uuid)
                elif cmd == "check_app":
                    r = host.check_app(id=args.uuid)
                elif cmd == "shutdown":
                    host.shutdown()
                elif cmd == "reboot":
                    host.reboot()
                    host.reconnect()
                    p = host.send_exmaple_provision_info()
                    print(f"Provisioning finished with {p}")
                elif cmd == "launch":
                    host.run_qemu()
                    host.reconnect()
                    p = host.send_exmaple_provision_info()
                    print(f"Provisioning finished with {p}")
                elif cmd == "invalid_json":
                    r = host.invalid_json()
                elif cmd == "exit":
                    sys.exit()

                print(f"Command returned: {r}")


if __name__ == "__main__":
    main()

