# Legacy OpenSCQ30 GUI

This directory contains the inherited libcosmic desktop client (`openscq30-gui`). OpenSCL4 desktop development and release packaging use the Slint client in [`desktop/`](../desktop/).

The `com.oppzippy.OpenSCQ30` desktop/AppStream resources, Windows product metadata, translations, installation recipes and release history in this directory belong to that legacy application. They are retained for provenance and maintenance of the old code; they do not describe current OpenSCL4 packages or distribution channels. The AppStream validation workflow validates this legacy file only.

Do not use the root `build-gui`, `build-gui-fast`, `install` or `uninstall` recipes to install current OpenSCL4. Use [the common build and release guide](../docs/build-release.md). The historically named `build-gui-installer` recipe already delegates to the current Windows release script.
