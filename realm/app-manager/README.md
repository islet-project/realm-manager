# app-manager

This crate implements a privileged realm service that allows for provisioning, verification, setup and launching of applications. It sets application runtime environment i.e. encrypted disk partitions for application images and their data. The encryption keys for applications and their data are derived using our key derivation shceme implemented by app-manager. This daemon is managed by Warden daemon using a dedicted protocol running via VSOCK.

