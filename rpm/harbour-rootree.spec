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
Version:    0.6.1
Release:    5
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
Requires:   libssl.so.3()(64bit)
Requires:   libcrypto.so.3()(64bit)
Requires:   libz.so.1()(64bit)
%else
Requires:   libgstreamer-1.0.so.0
Requires:   libgstbase-1.0.so.0
Requires:   libgstapp-1.0.so.0
Requires:   libgstvideo-1.0.so.0
Requires:   libgobject-2.0.so.0
Requires:   libpoppler-cpp.so.2
Requires:   libssl.so.3
Requires:   libcrypto.so.3
Requires:   libz.so.1
%endif

%package root
Summary:    Root mode for RooTree
Requires:   %{name} = %{version}-%{release}
# devel-su: la modalità sviluppatore (la sua password apre la modalità root)
Requires:   /usr/bin/devel-su

%description root
The helper that RooTree starts with devel-su when root mode is turned on:
it performs the file operations as root and talks only to the app over a
private socket. The app itself never runs as root.

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
# non setuid: lo avvia devel-su, dopo la password di sviluppatore
install -D -m0755 %{stagingdir}/rootree-helper %{buildroot}%{_libexecdir}/%{name}/rootree-helper

%files
%defattr(-,root,root,-)
%license %{_defaultlicensedir}/%{name}/LICENSE
%license %{_defaultlicensedir}/%{name}/NOTICE.md
%{_bindir}/%{name}
%{_datadir}/%{name}
%{_datadir}/applications/%{name}.desktop
%{_datadir}/icons/hicolor/*/apps/%{name}.png

%files root
%defattr(-,root,root,-)
%dir %{_libexecdir}/%{name}
%{_libexecdir}/%{name}/rootree-helper

%changelog
* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.1-5
- The connections file is reloaded when it changes on disk, instead of
  being overwritten later with the stale copy kept in memory.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.1-4
- "Remote options" submenu in the menu: start folder, edit the connection
  and disconnect (when inside a server; then it opens by itself), add a
  connection and lock the password vault.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.1-3
- In the start folder of a server the menu row is no longer greyed out: it
  shows a check mark and explains how to change it.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.1-2
- "Always open from this folder" in the menu while browsing a server makes
  the current folder the connection's start folder; the wizard summary has
  the start folder field too (servers imported from ~/.ssh/config skip the
  Server page, so they had no way to set it).

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.1-1
- Phase 5, step 2: SFTP connections work. Tapping a saved connection
  connects (password or SSH key, also with a passphrase) and opens its
  start folder, browsable like a local one (read-only for now: copying
  comes in the next step). The server fingerprint is checked against
  ~/.ssh/known_hosts like ssh does: a new server asks for confirmation, a
  changed one is refused. "Disconnect" in the connection menu.
- Password vault: passwords can be remembered, encrypted with a master
  password of your choice (Argon2id + XChaCha20-Poly1305), opened once per
  session; "Lock the password vault" in the menu.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.0-3
- Connection wizard: the servers defined in ~/.ssh/config (if it exists)
  are offered at the top of the first step; tapping one fills in an SFTP
  connection (address, port, user, key) and goes to the summary.
- SSH keys are shown as cards with their type, comment and the
  ~/.ssh/config servers that use them; with more than one key none is
  preselected, so the right one is chosen on purpose.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.0-2
- SSH key in the connection wizard: besides typing the path (also with ~/),
  "Browse…" opens a file chooser (hidden files shown, private keys marked),
  and the private keys found in ~/.ssh, recognised by their content, are
  offered as buttons. Warns about a public key or a file that is not a key.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.6.0-1
- Phase 5, step 1: remote connections interface. A "Network and cloud"
  section in the storage menu lists the saved connections; "Add a
  connection…" opens a step-by-step wizard (type, server, sign-in, name)
  for SFTP, FTP/FTPS, WebDAV, Nextcloud/ownCloud, SMB, S3, Google Drive,
  OneDrive, Dropbox and Mega. Connections can be edited, duplicated and
  deleted. Passwords are never written to the profiles file; connecting
  comes in the next step.

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.5.0-1
- Root mode (new harbour-rootree-root package): turned on from the menu with
  the developer password, it starts a separate helper through devel-su; the
  app never runs as root. A red band shows while it is on; it stops when
  turned off or when RooTree closes.
- In root mode: browse, copy, move, delete, rename and create anywhere;
  change permissions, owner and group. Files created as root inside a folder
  owned by the user stay the user's.
- The text viewer becomes an editor ("Edit"): saving never leaves a file
  half written, and as root the original is kept in
  /home/debugging/harbour-rootree (root only, latest copy per file).

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.4.0-3
- Rename / New folder: two selection handles to pick with a finger which
  part of the name to select (double tap selects a word).

* Sun Nov 08 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 0.4.0-2
- Rename / New folder: buttons to select all (or the name without its
  extension), copy and paste; the selection stays while tapping them.

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
