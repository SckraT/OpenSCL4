# Android interface

The Android interface uses three main destinations: Device, Presets and Settings.
Device pages are built from the settings exposed by the active device. Liberty 4
Pro has a dedicated illustration; unsupported features are omitted for other
models. **All device settings** retains access to every native category, including
capabilities that do not have a dedicated page.

## Navigation and connection

Back from a device subpage returns to its parent. Back from the device overview
opens the paired-device list and keeps the foreground connection active. Closing
the activity also leaves the service running. The device menu and notification
provide an explicit Disconnect action. Selecting the connected device again
reuses its connection; selecting another device cancels the previous connection
before opening the next one. Removing the active saved device also closes its
control connection; removing another saved device leaves the active connection alone.

Connection failures retain a Retry action. When Bluetooth is disabled, the
failure screen offers Android Bluetooth settings. Demo devices remain available
for interface testing without headphones.

## Device values

The native library supplies option labels, ranges, precision and current values.
Capability enumeration and setting reads run on a background dispatcher, so a
slow native query does not block the collecting UI thread during recreation.
The overview displays a loading indicator until its first setting snapshot arrives.
Battery fractions preserve the model's resolution, including coarse case charge
levels. Equalizer edits send one complete vector when a drag finishes; numeric
entry validates the device range and decimal precision before applying it.
Other numeric sliders also commit when the gesture finishes, so leaving a page
cannot cancel a delayed write. Multi-select labels toggle their complete row.
Preset matching is determined from the enabled fields and current device values.
The separate last-successfully-applied mark is recorded only after native
activation succeeds and is cleared on disconnect; it does not prove the current
values still match. Changing equalizer values retains the native spatial-audio side effects.
Settings shared by several native categories appear once in the editor. New
presets capture each setting once; toggling a field in older presets updates all
stored occurrences of that setting.

Multipoint disconnection and forgetting a saved device remain distinct actions.
Case features represent the controls actually exposed by the device protocol.

## Appearance and accessibility

The default palette uses violet accents, warm backgrounds and sage device cards.
System wallpaper colors remain an optional preference, and existing explicit
preferences are preserved. Light, dark and system appearance are supported.
Onest is bundled under the SIL Open Font License, included in application assets.
Interactive controls use Material semantics and at least 48 dp touch targets.
Large text removes the decorative overview illustration and stacks battery cards;
device pages and notification-slot settings can scroll.

## Validation

Unit tests cover connection ownership, binding and retry behavior, equalizer
precision, preset matching, range quantization, battery fractions and UI
responsiveness while synchronous native reads are blocked.
`SoundNavigationTest` exercises real demo-device writes and connection-preserving
Back navigation. Existing device-setting, migration and notification tests use
the new interface paths.

Build one architecture with `assembleArm64-v8aDebug` for an arm64 phone or
`assembleX86_64Debug` for an x86_64 emulator. Run Android instrumentation with
`connectedX86_64DebugAndroidTest`, or `connectedArm64-v8aDebugAndroidTest` on an
arm64 phone. Emulator demo checks do not establish physical Bluetooth behavior or
Pixel gesture-animation behavior; those require a phone and headphones.

A connected test task fails when a device ran no tests or no results were
written, including a test filter that matches nothing. AGP reports an
instrumentation that crashes before its first test as a passing run with zero
tests, so the build checks the result files itself; read the crash with
`adb logcat -b crash -d` on that device.

Software emulators can pass `-e uiTimeoutMillis 60000` to the instrumentation
runner to extend asynchronous waits. The default timeouts and state assertions
remain unchanged on regular devices.
