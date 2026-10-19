# Capture Bluetooth Logs on Android

1. [Enable developer options](https://developer.android.com/studio/debug/dev-options)
2. Turn off bluetooth
3. In developer options, set "Enable Bluetooth HCI snoop log" to Enabled.
4. Turn on bluetooth and connect to your device. Don't have any audio playing to avoid unnecessary data in the log.
5. Open the app for your device and perform actions
6. When you're done, in developer options, generate a bug report and then turn off "Enable Bluetooth HCI snoop log"
7. In the bug report zip file, the log is located at one of these paths, depending on your Android version: `FS/data/misc/bluetooth/logs/btsnoop_hci.log`, `FS\data\log\bt\btsnoop_hci.log`

## What data is collected

Bluetooth logs can contain private information. Review the capture and attach only the relevant, redacted material to [OpenSCL4 Issues](https://github.com/SckraT/OpenSCL4/issues). If a capture cannot be shared publicly, ask the fork maintainer for an agreed private route before sending it; this project does not designate the upstream author's email as its support channel.

### General

- the name and mac address of your phone
- the name and mac address of your bluetooth device

### Soundcore Specific Data

- serial number
- firmware version
- if the device supports dual connections, the names and mac addresses of all devices listed in dual connections
- Various statistics about how much certain features of the device were used (things like number of times powered on/off, button press counts, etc.)
