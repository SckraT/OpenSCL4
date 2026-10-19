# Requesting support for a new Soundcore device

Search [OpenSCL4 Issues](https://github.com/SckraT/OpenSCL4/issues) before opening a new request. Include the exact model number, firmware, OS and OpenSCL4 version/commit. The shared library includes a development model for reading the initial state packet. Review the [capture privacy notes](capture-bluetooth-logs.md#what-data-is-collected) before sharing packets or logs.

## Desktop (Slint)

Open Add Device, choose the development-information model (internally `SoundcoreDevelopment`), then select and connect the requested Bluetooth device. Its exposed settings include State Update Packet; copy that value. The development model gathers protocol information and does not establish support for controlling the device.

## Android

Add the device using the development-information model and grant the required Bluetooth permissions. Connect, open All device settings, then Device Information, and copy State Update Packet. Display names can vary by language; the internal model identifier is `SoundcoreDevelopment`.

## CLI

The retained CLI executable is still `openscq30`. Set `OPENSCQ30_DATABASE_PATH` to an isolated database if you do not want to use its legacy default data directory. These commands use a system-paired device:

```bash
openscq30 paired-devices add --mac-address YOUR_SOUNDCORE_DEVICE_MAC_ADDRESS --model SoundcoreDevelopment
openscq30 device --mac-address YOUR_SOUNDCORE_DEVICE_MAC_ADDRESS setting --get stateUpdatePacket --json
```
