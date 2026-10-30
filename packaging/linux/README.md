# OpenSCL4 for Linux

Experimental target: x86_64 Ubuntu 24.04/26.04 with GNOME and Kubuntu 24.04/26.04 with KDE Plasma. Packages and CI checks are implemented; the full session matrix and real-headphone QA still have to pass before a stable release. See `docs/build-linux.md` and `docs/build-release.md` in the source repository.

Install the `.deb` with `sudo apt install ./openscl4_VERSION_amd64.deb`. APT installs its system dependencies. Pair the headphones in the system Bluetooth settings, then open OpenSCL4 from the application launcher.

For the `.tar.gz`, extract the archive and run `./openscl4-VERSION/usr/bin/openscl4`. This archive targets the same Ubuntu versions and requires the same runtime packages listed in `packaging/linux/control` in the source repository; it is not a self-contained distribution-independent runtime.

The AppImage is a preview format: the `.deb` stays the main package. Make it executable with `chmod +x openscl4-VERSION-linux-x86_64.AppImage` and run it. It contains the same application files as the other packages and needs the same runtime packages, including glibc 2.39 or newer. It mounts itself with FUSE through `fusermount3` from the `fuse3` package; where FUSE is unavailable, run it with `--appimage-extract-and-run`. It does not update itself or add a launcher entry: to update, download the newer AppImage and check it against `SHA256SUMS`.

GNOME can use an AppIndicator extension for the tray icon. The implemented behavior is to exit on close without a working tray and restore a hidden window if its tray disappears. This still needs verification on the target sessions. The extension is optional.

OpenSCL4 stores its data under the system configuration directory in `openscl4/`. It installs alongside OpenSCQ30 and does not import its data automatically.

Source and issue reports: https://github.com/SckraT/OpenSCL4

Based on OpenSCQ30 by Kyle Scheuing and contributors: https://github.com/Oppzippy/OpenSCQ30

Licensed under GPL-3.0-or-later; see `copyright` alongside this file.
