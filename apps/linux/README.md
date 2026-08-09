# DroidHarbor Linux app (future development)

Rust + GTK4/libadwaita binary linking `dh-domain` directly, with no FFI layer.
Tray via StatusNotifierItem, XDG Desktop Portal folder picker,
`org.freedesktop.Notifications`, `org.freedesktop.FileManager1` reveal.

The data and domain layers are kept Linux-portable (and CI-checked on Ubuntu)
from day one so this app is purely UI work when it starts.
