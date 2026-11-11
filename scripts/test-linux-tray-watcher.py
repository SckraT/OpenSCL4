"""Stand-in for the desktop's StatusNotifierWatcher on the smoke test's session bus."""

import time

import dbus
import dbus.service
from dbus.mainloop.glib import DBusGMainLoop
from gi.repository import GLib


WATCHER = "org.kde.StatusNotifierWatcher"
PROPERTIES = "org.freedesktop.DBus.Properties"


class Watcher(dbus.service.Object):
    def __init__(self, bus_name):
        super().__init__(bus_name, "/StatusNotifierWatcher")
        self.items = []
        self.first_read = None

    @dbus.service.method(WATCHER, in_signature="s", out_signature="")
    def RegisterStatusNotifierItem(self, service):
        self.items.append(f"{service}/StatusNotifierItem")
        self.first_read = None
        print(f"registered {service}", flush=True)

    def _observe_registration(self):
        if not self.items:
            return
        now = time.monotonic()
        if self.first_read is None:
            self.first_read = now
        elif now - self.first_read >= 1:
            # The app polls every two seconds. Let it process a full poll before closing the window.
            print("registered tray observed", flush=True)

    def _properties(self):
        return {
            "IsStatusNotifierHostRegistered": dbus.Boolean(True),
            "RegisteredStatusNotifierItems": dbus.Array(self.items, signature="s"),
            "ProtocolVersion": dbus.Int32(0),
        }

    @dbus.service.method(PROPERTIES, in_signature="ss", out_signature="v")
    def Get(self, interface, property_name):
        if interface == WATCHER and property_name == "RegisteredStatusNotifierItems":
            self._observe_registration()
        return self._properties()[property_name]

    @dbus.service.method(PROPERTIES, in_signature="s", out_signature="a{sv}")
    def GetAll(self, interface):
        if interface != WATCHER:
            return {}
        self._observe_registration()
        return self._properties()


if __name__ == "__main__":
    DBusGMainLoop(set_as_default=True)
    watcher = Watcher(dbus.service.BusName(WATCHER, bus=dbus.SessionBus()))
    GLib.MainLoop().run()
