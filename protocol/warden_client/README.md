# warden daemon <=> cmd client protocol (client_lib)

This crate implements the protocol used to enable communication between the daemon and the API client. It provides messages allowing for:
* defining a realm
* defining an application inside a realm
* constrolling the realm lifetime:
    * starting a realm
    * stopping a realm
    * rebooting a realm
* controlling application lifetime
    * starting an application
    * stopping an application
* performing application update
