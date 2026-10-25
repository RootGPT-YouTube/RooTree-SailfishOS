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
Version:    0.2.0
Release:    1
Summary:    RooTree — file manager for Sailfish OS
License:    GPLv3+
Group:      Applications/System
URL:        https://github.com/RootGPT-YouTube/RooTree-SailfishOS
ExclusiveArch: aarch64 armv7hl
AutoReqProv: no
Requires:   rootgpt-qt6-runtime >= 6.12.0

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
