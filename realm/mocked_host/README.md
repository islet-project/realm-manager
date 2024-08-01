# Mocked host

This python script was created to enable testing the realm daemon (app-manager) without the rest of this setup i. e. the warden daemon.

## Setting up
Ensure you set up networking as described in [Previous README](https://github.com/islet-project/realm-manager/tree/main/realm#starting-qemu-with-networking-support) and built the kernel image with QEMU by running `make compile`. After that start the `run.py` script. The scripts takes a couple of arguments:
| Name | Description | Default value |
|-|-|-|
|--vsock-port| Listen port, ensure that the app-manager has the same number in config | 1337|
|--guest-cid| The VSOCK guest address | 1337|
|--tap-device | Tap used to provide networking | tap100|
|--qemu-serial | Arguments passed to QEMU's `-serial` oprion, use either `tcp:<ip>:<port>` and then `nc -lvp <port>` or `file:<path>` and `tail -f <path>` to read logs from the realm | tcp:localhost:1337|
|--test | Run some simple tests that start, stop, kill and reboot stuff | N/A |

## Controlling the realm
After the realm has been launched you should see this on the terminal (using tail -f in my case):
```
[    2.320099] Warning: unable to open an initial console.
[    2.365938] Freeing unused kernel memory: 36352K
[    2.367138] Run /init as init process
Running mdev -s
Running ifup -a
[    3.815278] e1000: eth0 NIC Link is Up 1000 Mbps Full Duplex, Flow Control: RX
udhcpc: started, v1.37.0.git
Setting IP address 0.0.0.0 on eth0
udhcpc: broadcasting discover
udhcpc: broadcasting select for 192.168.100.237, server 192.168.100.1
udhcpc: lease of 192.168.100.237 obtained from 192.168.100.1, lease time 3600
Setting IP address 192.168.100.237 on eth0
Deleting routers
route: SIOCDELRT: No such process
Adding router 192.168.100.1
Recreating /etc/resolv.conf
 Adding DNS server 192.168.100.1

Please press Enter to activate this console. 2024-07-31T11:38:47.483Z INFO  [app_manager] Reading config file: "/etc/app-manager/config.yml"
2024-07-31T11:38:47.569Z INFO  [app_manager::manager] Connected to warden daemon
2024-07-31T11:38:47.572Z INFO  [app_manager] Provishioning...
2024-07-31T11:38:47.573Z INFO  [app_manager::manager] Waiting for provision info
2024-07-31T11:38:47.590Z DEBUG [app_manager::manager] Received provision info: [ApplicationInfo { id: 5d63a211-e8aa-4179-ac22-af7e843a3f43, name: "Test app", version: "1.0", image_registry: "http://registry.com", image_part_uuid: 0178460a-ffea-4674-8e17-8530758c4c2e, data_part_uuid: 259c493c-e5d2-4fd4-a962-7efd45a0bd91 }]
2024-07-31T11:38:47.593Z INFO  [app_manager::manager] Starting installation
2024-07-31T11:38:47.637Z DEBUG [devicemapper::core::dm] Creating device crypt_0178460a-ffea-4674-8e17-8530758c4c2e (uuid=None)
2024-07-31T11:38:47.667Z DEBUG [devicemapper::core::dm] Resuming device crypt_0178460a-ffea-4674-8e17-8530758c4c2e
[    5.389424] EXT4-fs (dm-0): mounting ext2 file system using the ext4 subsystem
[    5.396872] EXT4-fs (dm-0): warning: mounting unchecked fs, running e2fsck is recommended
[    5.407595] EXT4-fs (dm-0): mounted filesystem 77af6112-6ce4-4ca2-b1d5-00bf98283103 r/w without journal. Quota mode: none.
[    5.408214] ext2 filesystem being mounted at /apps/5d63a211-e8aa-4179-ac22-af7e843a3f43/image supports timestamps until 2038-01-19 (0x7fffffff)
2024-07-31T11:38:47.718Z INFO  [app_manager::app] Installing application
2024-07-31T11:38:47.867Z DEBUG [devicemapper::core::dm] Creating device crypt_259c493c-e5d2-4fd4-a962-7efd45a0bd91 (uuid=None)
2024-07-31T11:38:47.875Z DEBUG [devicemapper::core::dm] Resuming device crypt_259c493c-e5d2-4fd4-a962-7efd45a0bd91
2024-07-31T11:38:47.879Z INFO  [app_manager::app] Mounting data partition
[    5.580489] EXT4-fs (dm-1): mounting ext2 file system using the ext4 subsystem
[    5.582859] EXT4-fs (dm-1): warning: mounting unchecked fs, running e2fsck is recommended
[    5.584870] EXT4-fs (dm-1): mounted filesystem 2e07ad5f-c2f9-40e3-9869-68f21676a2a9 r/w without journal. Quota mode: none.
[    5.585291] ext2 filesystem being mounted at /apps/5d63a211-e8aa-4179-ac22-af7e843a3f43/data supports timestamps until 2038-01-19 (0x7fffffff)
2024-07-31T11:38:47.887Z INFO  [app_manager::app] Mounting overlayfs
2024-07-31T11:38:47.929Z INFO  [app_manager::manager] Finished installing 5d63a211-e8aa-4179-ac22-af7e843a3f43
2024-07-31T11:38:47.930Z INFO  [app_manager::manager] Provisioning finished
2024-07-31T11:38:47.938Z INFO  [app_manager::manager] Starting 5d63a211-e8aa-4179-ac22-af7e843a3f43
2024-07-31T11:38:47.978Z INFO  [app_manager] Applications started entering event loop
2024-07-31T11:38:47.999Z INFO  [app_manager::launcher::handler] Application stdout: I'm alive

2024-07-31T11:38:48.997Z INFO  [app_manager::launcher::handler] Application stdout: I'm alive
```

And on the terminal with the `run.py` script:
```
Waiting for connection from realm
WARNING: Image format was not specified for 'disk.raw' and probing guessed raw.
         Automatically detecting the format is dangerous for raw images, write operations on block 0 will be restricted.
         Specify the 'raw' format explicitly to remove the restrictions.
Accepted connection from (1337, 717899597)
Provisioning finished with {'Success': []}
> 
```

This means that the realm is up and han successfully installed and started the applications. From now on you can use the follwing commands (typed after the `> `):
|Name| Description |
|-|-|
|start_app | Start the application by id (default value) |
|stop_app | Stop (SIGTERM) the application by id (default value)|
|kill_app | Kill (SIGKILL) the application by id (default value)|
|check_app | Check if the application is running by id (default value)|
|reboot| Shutdown all applications and reboot the realm |
|shutdown | Shutdown the applications and then the realm |
|launch | Relaunch QEMU, usefull after issuing the shutdown command |
|invalid_json | Send invalid request to test if it behaves as designed |
|exit | Perform `sys.exit()` and exit from the `run.py` script. |
