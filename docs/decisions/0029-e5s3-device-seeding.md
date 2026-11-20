# 0029 — E5.S3 Development device library seeding

The three-second hold on the Library eyebrow opens the native file picker
in debug builds. The Rust importer reads selected audio with `mt-meta` and
copies it beside `device.sqlite`. The display reads its counts from the
same database. `seed-device` uses the same importer to build a portable
database and music directory for Android or iOS development.

The destination is based on the content hash and extension. Importing the
same content again preserves its track ID. Files are published atomically,
and a database rollback removes only the files created in that import.

Cargokit enables the Rust seeder only in the debug configuration. The
Android release APK retained a seeded library and the Library hold left
the display in place. The release native library contained the debug-build
error branch and excluded the seeder.

The Android emulator opened a database transferred with ADB, then added a
second file through the native picker. A Dart integration test also checked
that a seeded track plays and seeks after the source file is removed.
Native iOS verification and hosted CI remain open.
