# Warden <=> Realm protocol

This crate implements the protocol used to control realms from the realm daemon. It features messages enabling:
* specyfing what application should be install in what versions (revisions)
* controlling application lifetime:
    * starting an application
    * stopping an application
* controlling realm lifetime:
    * performing a reboot
    * halting a realm
