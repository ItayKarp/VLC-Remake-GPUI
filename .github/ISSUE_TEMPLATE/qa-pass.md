---
name: QA pass
about: Record a release's run of the OS and display matrix (docs/qa.md)
title: "QA pass: v"
labels: qa
---

**Version:** v
**Corpus:** the archive `tools/corpus/corpus.lock` pins, or `make.sh` of <date>

For each row run, tick it and note the OS build, the package and anything found (an issue
link for each bug). Rows not run stay unticked and go in the release notes.

### OS matrix (S9.8.2): corpus tests and the smoke pass

- [ ] Windows 10 22H2, x64, MSI
- [ ] Windows 11, x64, MSI
- [ ] Windows 11, arm64, MSI (x64, emulated)
- [ ] macOS 13 Ventura, arm64
- [ ] macOS 13 Ventura, x64
- [ ] macOS 14 Sonoma, arm64
- [ ] macOS 15 or later, arm64
- [ ] macOS 15 or later, x64
- [ ] Ubuntu LTS, X11, .deb
- [ ] Ubuntu LTS, Wayland, AppImage
- [ ] Ubuntu LTS, Wayland, Flatpak
- [ ] Fedora, Wayland, PipeWire, .rpm
- [ ] Fedora, Wayland, PipeWire, Flatpak

### Display matrix (S9.8.3)

- [ ] 100 % scaling
- [ ] 125 % scaling
- [ ] 150 % scaling
- [ ] 200 % scaling
- [ ] Mixed DPI, two monitors
- [ ] An HDR monitor
- [ ] A 1366×768 laptop

### Found
