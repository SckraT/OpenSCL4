# Soundcore Device Faker

This inherited development tool emulates device responses for protocol investigation. It is not a packaged OpenSCL4 client or a real-headphone compatibility test. See [device development](../../docs/device-development.md).

## Running Locally

A validated local setup is not documented here; the retained recipes below use a remote Linux host.

## Running Remotely

The inherited workflow uses a remote Raspberry Pi when the local Bluetooth adapter is unsuitable for Bumble. This is an upstream development setup, not a newly validated hardware recommendation for OpenSCL4.

### Setup

Copy `.env.example` to `.env`. Set `SOUNDCORE_DEVICE_FAKER_REMOTE_USER` to your username on the remote machine and `SOUNDCORE_DEVICE_FAKER_REMOTE_HOST` to the address of the remote machine.

`SOUNDCORE_DEVICE_FAKER_REMOTE_TRANSPORT_SPEC` tells [Bumble](https://github.com/google/bumble) what bluetooth adapter to use. The value in `.env.example` is appropriate for a Raspberry Pi 3 Model B, and presumably other Raspberry Pis too. See [https://google.github.io/bumble/transports/index.html](https://google.github.io/bumble/transports/index.html) for information on selecting a transport.

Once `.env` is set up, run `just init-remote` to copy over the source files, create a venv, and install dependencies with pip.

### Running

To run on the configured remote machine, use `just run-remote a3004`, replacing a3004 with the name of your device configuration file excluding .toml. Whenever the file is modified, responses will be reloaded automatically. This does not affect the name or rfcomm uuid.

In order to keep files on the remote machine up to date while editing locally, use `just sync-remote-watch`. Files will be synchronized whenever anything changes.
