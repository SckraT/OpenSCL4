# OpenSCL4 for Linux

Supported initially: x86_64 Ubuntu 24.04/26.04 with GNOME and Kubuntu 24.04/26.04 with KDE Plasma. See the session matrix in the cross-platform release plan.

Install the `.deb` with `sudo apt install ./openscl4_VERSION_amd64.deb`. APT installs its system dependencies. Pair the headphones in the system Bluetooth settings, then open OpenSCL4 from the application launcher.

For the `.tar.gz`, extract the archive and run `./openscl4-VERSION/usr/bin/openscl4`. This archive targets the same Ubuntu versions and requires the same runtime packages listed in `packaging/linux/control` in the source repository; it is not a self-contained distribution-independent runtime.

GNOME can use an AppIndicator extension for the tray icon. Without a working tray, closing the window exits the application. If the tray disappears while the window is hidden, the window reopens. The extension is optional.

OpenSCL4 stores its data under the system configuration directory in `openscl4/`. It installs alongside OpenSCQ30 and does not import its data automatically.

Source and issue reports: https://github.com/SckraT/OpenSCL4

Based on OpenSCQ30 by Kyle Scheuing and contributors: https://github.com/Oppzippy/OpenSCQ30

Licensed under GPL-3.0-or-later; see `copyright` alongside this file.
