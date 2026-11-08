# harbour-rootree — file manager per Sailfish OS (OpenRepos, non Jolla Store).
# Si impacchetta uno staging già pronto (tools/pacchetto.sh), qui non si compila.
# Qt arriva dal runtime condiviso rootgpt-qt6-runtime (/home/rootgpt-qt6).
# Launcher /usr/bin/harbour-rootree: sailjail rifiuta gli script e vuole un ELF
# in /usr/bin; il .desktop ha [X-Sailjail] Sandboxing=Disabled, altrimenti un
# file manager vedrebbe solo le sue cartelle.

%global debug_package %{nil}
%define __os_install_post %{nil}
%define __brp_strip %{nil}
%define __brp_strip_static_archive %{nil}
%define __brp_strip_comment_note %{nil}

Name:       harbour-rootree
Version:    0.4.0
Release:    1
Summary:    RooTree — file manager for Sailfish OS
License:    GPLv3+
Group:      Applications/System
URL:        https://github.com/RootGPT-YouTube/RooTree-SailfishOS
ExclusiveArch: aarch64 armv7hl
AutoReqProv: no
Requires:   rootgpt-qt6-runtime >= 6.12.0
# anteprime (GStreamer per i video, poppler per i PDF) e «Apri con»
# (libcontentaction): pacchetti standard di Sailfish OS. AutoReqProv è spento,
# quindi le librerie si chiedono a mano, per soname.
Requires:   gstreamer1.0-plugins-base
Requires:   /usr/bin/lca-tool
# RAR e formati rari (iso, cpio, cab...): libarchive, caricata a runtime con
# dlopen (zip, tar.* e 7z invece sono nel binario)
Requires:   libarchive
%ifarch aarch64
Requires:   libgstreamer-1.0.so.0()(64bit)
Requires:   libgstbase-1.0.so.0()(64bit)
Requires:   libgstapp-1.0.so.0()(64bit)
Requires:   libgstvideo-1.0.so.0()(64bit)
Requires:   libgobject-2.0.so.0()(64bit)
Requires:   libpoppler-cpp.so.2()(64bit)
%else
Requires:   libgstreamer-1.0.so.0
Requires:   libgstbase-1.0.so.0
Requires:   libgstapp-1.0.so.0
Requires:   libgstvideo-1.0.so.0
Requires:   libgobject-2.0.so.0
Requires:   libpoppler-cpp.so.2
%endif

%description
RooTree is a file manager for Sailfish OS: internal memory, Android storage,
SD card, cloud and remote storage, archives, and an optional root mode.

%prep

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}%{_datadir}/%{name} %{buildroot}%{_bindir}
cp -a %{stagingdir}/app/. %{buildroot}%{_datadir}/%{name}/
install -m0755 %{stagingdir}/launcher %{buildroot}%{_bindir}/%{name}
install -D -m0644 %{stagingdir}/%{name}.desktop %{buildroot}%{_datadir}/applications/%{name}.desktop
for s in 86 108 128 172 256; do
  install -D -m0644 %{stagingdir}/icons/${s}x${s}/%{name}.png \
    %{buildroot}%{_datadir}/icons/hicolor/${s}x${s}/apps/%{name}.png
done
install -D -m0644 %{stagingdir}/LICENSE %{buildroot}%{_defaultlicensedir}/%{name}/LICENSE
install -D -m0644 %{stagingdir}/NOTICE.md %{buildroot}%{_defaultlicensedir}/%{name}/NOTICE.md

%files
%defattr(-,root,root,-)
%license %{_defaultlicensedir}/%{name}/LICENSE
%license %{_defaultlicensedir}/%{name}/NOTICE.md
%{_bindir}/%{name}
%{_datadir}/%{name}
%{_datadir}/applications/%{name}.desktop
%{_datadir}/icons/hicolor/*/apps/%{name}.png

%changelog
* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.4.0-1
- Search from the current folder down: by name (also with * and ?), by
  regular expression, or inside the files' content, with results arriving
  while it runs.
- Archives: zip, tar (.gz .bz2 .xz .zst) and 7z open like folders, and
  copying from inside one extracts just those items; "Extract here" and
  "Compress…" in the selection menu. RAR and other read-only formats through
  the system libarchive. Unsafe archive paths are never written outside.
- Sorting by date now shows the newest first; a new "Only for this folder"
  switch (on by default) keeps the chosen sorting for the current folder
  only, while turned off it applies everywhere.

* Tue Oct 27 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.3.1-2
- Classic SailfishOS theme: file icons are tinted too, in the secondary
  highlight colour of the ambience.

* Tue Oct 27 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.3.1-1
- New theme choice in the menu: RooTundra (steel, default) or classic
  SailfishOS (flat, colours and font of the current ambience).
- Every storage in the places menu can be expanded to its standard folders
  (Documents, Downloads, Pictures, Music, Videos); the menu always opens
  collapsed.
- The drop-down arrow next to the storage button is no longer covered.

* Sun Oct 25 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.3.0-1
- Phase 2, open and view: previews of images (also WebP and TIFF), videos
  and PDFs, kept in the standard ~/.cache/thumbnails; built-in image viewer
  (swipe, pinch and double tap zoom) and text viewer.
- Open with the Sailfish apps (default app, "Open with" with the app icons,
  "always use for this type") and share with the system share dialog.
- Files outside the folders that sandboxed Sailfish apps can see are handed
  over through a hard link in ~/Downloads/.rootree, removed after a day.

* Sun Oct 25 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.2.0-1
- Phase 1, local file browser in the style of Dolphin: storage drop-down
  menu (internal, Android, SD card/USB, file system, bookmarks), breadcrumb
  path, back/forward/up, icon and details views, sorting, hidden files.
- New folder, rename, delete, properties with permissions, multiple
  selection, copy/cut/paste with a queue, progress, cancel, conflict and
  error questions.
- RooTundra look: steel panels and buttons, electric blue LEDs, ice and
  frost, steel file icons, Rajdhani font.

* Sun Oct 25 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.1.0-1
- First build (phase 0): lists a local folder through the Rust core.
