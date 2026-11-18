/* Launcher ELF di RooTree (e delle app Qt6 di RootGPT con un Qt 6 incluso).
 *
 * Dalla 0.9.7 RooTree porta il suo Qt 6.12 dentro il pacchetto, in
 * /home/rootree/qt6 (prima era il pacchetto a parte rootgpt-qt6-runtime, in
 * /home/rootgpt-qt6): RUNTIME dice dove.
 *
 * sailjail rifiuta gli script e vuole come Exec un ELF in /usr/bin: questo
 * programma imposta l'ambiente del Qt dell'app e fa execv del
 * binario vero dell'app, con argv[0] forgiato = percorso del launcher (così
 * jolla-home aggancia la cover: cerca un processo la cui cmdline combaci con
 * la riga Exec del .desktop). È lo stesso schema di rootitanium-launch.c di
 * RooTitanium, senza la parte del motore web.
 *
 * Si compila una volta per app:
 *   gcc -O2 -DAPP_EXEC='"/home/rootree/app/harbour-rootree"' \
 *       -DAPP_ID='"rootree"' -DRUNTIME='"/home/rootree/qt6"' -o harbour-rootree launcher.c
 *
 * Diagnostica sul campo: se esiste ~/.config/rootgpt/<APP_ID>.debug, stdout e
 * stderr vanno in /tmp/<APP_ID>.log e i log Qt arrivano anche dai lanci da
 * icona. Il file lo crea l'utente che segnala un problema.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <limits.h>

#ifndef APP_EXEC
#error "definire APP_EXEC (percorso del binario dell'app)"
#endif
#ifndef APP_ID
#error "definire APP_ID (nome breve dell'app, per il log)"
#endif

#ifndef RUNTIME
#error "definire RUNTIME (cartella del Qt dell'app, es. /home/rootree/qt6)"
#endif

#if defined(__aarch64__)
#define RT_LIB "lib64"
#else
#define RT_LIB "lib"
#endif

int main(int argc, char **argv)
{
    (void)argc;
    {
        const char *home = getenv("HOME");
        char dbg[PATH_MAX];
        snprintf(dbg, sizeof(dbg), "%s/.config/rootgpt/" APP_ID ".debug", home ? home : "");
        if (home && access(dbg, F_OK) == 0
            && freopen("/tmp/" APP_ID ".log", "w", stdout)
            && freopen("/tmp/" APP_ID ".log", "a", stderr)) {
            setvbuf(stdout, NULL, _IOLBF, 0);
            setvbuf(stderr, NULL, _IOLBF, 0);
            setenv("QT_LOGGING_RULES", "qt.qpa*=true", 1);
            /* senza questo i console.log del QML non arrivano a stderr quando
             * stderr non è una tty */
            setenv("QT_FORCE_STDERR_LOGGING", "1", 1);
            fprintf(stderr, "[launcher] diagnostica attiva (%s)\n", dbg);
        }
    }

    /* prima il Qt dell'app, poi le librerie di sistema e quelle android della HAL
     * grafica (senza, libhybris non trova gralloc & co.) */
    {
        const char *old = getenv("LD_LIBRARY_PATH");
        char buf[PATH_MAX * 3];
        snprintf(buf, sizeof(buf),
                 RUNTIME "/lib:/usr/" RT_LIB ":/usr/libexec/droid-hybris/system/" RT_LIB
                 ":/system/" RT_LIB ":/vendor/" RT_LIB ":/odm/" RT_LIB "%s%s",
                 old && *old ? ":" : "", old ? old : "");
        setenv("LD_LIBRARY_PATH", buf, 1);
    }
    setenv("QT_PLUGIN_PATH", RUNTIME "/plugins", 1);
    setenv("QT_QPA_PLATFORM_PLUGIN_PATH", RUNTIME "/plugins/platforms", 1);
    setenv("QML2_IMPORT_PATH", RUNTIME "/qml", 1);
    setenv("QML_IMPORT_PATH", RUNTIME "/qml", 1);

    /* La sessione esporta variabili per il Qt5 patchato di Sailfish che il Qt6
     * upstream legge davvero (verificato da RooTitanium il 21/07/2026):
     * QT_QPA_PLATFORM=wayland (plugin generico, va imposto wayland-egl),
     * QT_WAYLAND_RESIZE_AFTER_SWAP (blocca la geometria), QMLSCENE_DEVICE. */
    setenv("QT_QPA_PLATFORM", "wayland-egl", 1);
    unsetenv("QT_WAYLAND_RESIZE_AFTER_SWAP");
    unsetenv("QMLSCENE_DEVICE");
    /* app a schermo intero: la decorazione le mangerebbe area utile */
    setenv("QT_WAYLAND_DISABLE_WINDOWDECORATION", "1", 1);
    {
        const char *xrd = getenv("XDG_RUNTIME_DIR");
        if (!xrd || !*xrd) {
            char buf[64];
            snprintf(buf, sizeof(buf), "/run/user/%u", (unsigned)getuid());
            setenv("XDG_RUNTIME_DIR", buf, 1);
            xrd = getenv("XDG_RUNTIME_DIR");
        }
        setenv("WAYLAND_DISPLAY", "wayland-0", 0);
        const char *dbus = getenv("DBUS_SESSION_BUS_ADDRESS");
        if (!dbus || !*dbus) {
            char buf[PATH_MAX];
            snprintf(buf, sizeof(buf), "unix:path=%s/dbus/user_bus_socket", xrd);
            setenv("DBUS_SESSION_BUS_ADDRESS", buf, 1);
        }
    }

    /* Tastiera di sistema (Maliit) se c'è maliit-server, altrimenti quella
     * di Qt. QT_IM_MODULE=Maliit della sessione non basta: Qt cerca un plugin
     * con quel nome e, non trovandolo, nessuna tastiera (RooTitanium, 6/09/2026). */
    if (access(RUNTIME "/plugins/platforminputcontexts/libmaliitplatforminputcontextplugin.so", R_OK) == 0
        && access("/usr/bin/maliit-server", X_OK) == 0)
        setenv("QT_IM_MODULE", "maliit", 1);
    else
        setenv("QT_IM_MODULE", "qtvirtualkeyboard", 1);

    /* solo se la sessione non lo passa: un ripiego neutro, non italiano */
    setenv("LANG", "en_US.UTF-8", 0);

    argv[0] = (char *)"/usr/bin/harbour-" APP_ID;
    execv(APP_EXEC, argv);
    perror("execv " APP_EXEC);
    return 127;
}
