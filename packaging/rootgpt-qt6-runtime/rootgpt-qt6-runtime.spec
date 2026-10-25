# rootgpt-qt6-runtime — Qt 6.12 condiviso fra le app Qt6 di RootGPT.
# Come RooTitanium: si impacchetta uno staging già pronto (assembla.py), qui
# non si compila niente. Build:
#   sb2 -t SailfishOS-5.1.0.11-<arch>.default rpmbuild -bb rootgpt-qt6-runtime.spec \
#       --define "_topdir <assoluto>/rpmbuild" --define "stagingdir <assoluto>"
# Il payload (~120 MB) va in /home/rootgpt-qt6: il rootfs dei telefoni è piccolo
# (un utente di RooTelegram è rimasto senza spazio per aggiornare).

%global _runtime /home/rootgpt-qt6
%global debug_package %{nil}
%define __os_install_post %{nil}
%define __brp_strip %{nil}
%define __brp_strip_static_archive %{nil}
%define __brp_strip_comment_note %{nil}

Name:       rootgpt-qt6-runtime
Version:    6.12.0
Release:    1
Summary:    Qt 6.12 runtime shared by RootGPT's Qt6 apps
License:    LGPLv3 and GPLv3 and BSD and MIT
Group:      System/Libraries
ExclusiveArch: aarch64 armv7hl
AutoReqProv: no
# Librerie di sistema che il runtime non porta (vedi SISTEMA in assembla.py),
# per soname. Elenco stampato da assembla.py il 25/10/2026.
%ifarch aarch64
Requires: libEGL.so.1()(64bit)
Requires: libGLESv2.so.2()(64bit)
Requires: libwayland-client.so.0()(64bit)
Requires: libwayland-cursor.so.0()(64bit)
Requires: libwayland-egl.so.1()(64bit)
Requires: libssl.so.3()(64bit)
Requires: libcrypto.so.3()(64bit)
Requires: libsqlite3.so.0()(64bit)
Requires: libsystemd.so.0()(64bit)
Requires: libstdc++.so.6()(64bit)
Requires: libgcc_s.so.1()(64bit)
%else
Requires: libEGL.so.1
Requires: libGLESv2.so.2
Requires: libwayland-client.so.0
Requires: libwayland-cursor.so.0
Requires: libwayland-egl.so.1
Requires: libssl.so.3
Requires: libcrypto.so.3
Requires: libsqlite3.so.0
Requires: libsystemd.so.0
Requires: libstdc++.so.6
Requires: libgcc_s.so.1
%endif

%description
Qt 6.12 (Core, Gui, Qml, Quick, Quick Controls, Svg, Wayland client, TLS)
for Sailfish OS, shared by the Qt6 apps of RootGPT so that each app does not
carry its own copy. Installed in /home/rootgpt-qt6.

%prep

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}%{_runtime}
cp -a %{stagingdir}/runtime/. %{buildroot}%{_runtime}/
mkdir -p %{buildroot}%{_defaultlicensedir}/%{name}
install -m0644 %{stagingdir}/NOTICE.md %{buildroot}%{_defaultlicensedir}/%{name}/NOTICE.md

%files
%defattr(-,root,root,-)
%license %{_defaultlicensedir}/%{name}/NOTICE.md
%{_runtime}

%changelog
* Sun Oct 25 2026 RootGPT <166707820+RootGPT-YouTube@users.noreply.github.com> - 6.12.0-1
- First package: Qt 6.12.0 built for Sailfish OS 5.1 (from RooTitanium's
  Qt 6.12 stack), with the Maliit keyboard plugin.
