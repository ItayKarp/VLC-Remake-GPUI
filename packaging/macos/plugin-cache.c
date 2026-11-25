/*
 * Writes libVLC's plugin cache (plugins.dat) for the plugins in VLC_PLUGIN_PATH, as VideoLAN's
 * vlc-cache-gen does; VLC.app doesn't ship that tool. build-app.sh runs it after signing, since
 * signing changes every plugin's size and time, which the cache records (S9.4.4).
 *
 * Usage: plugin-cache <libvlccore.dylib> <libvlc.dylib>   (with VLC_PLUGIN_PATH set)
 */
#include <dlfcn.h>
#include <stdio.h>

typedef void *(*libvlc_new_fn)(int, const char *const *);
typedef void (*libvlc_release_fn)(void *);

int main(int argc, char **argv) {
    if (argc != 3) {
        fprintf(stderr, "usage: %s <libvlccore> <libvlc>\n", argv[0]);
        return 2;
    }
    /* libvlc finds the core by its @rpath name, so the core is loaded first. */
    void *core = dlopen(argv[1], RTLD_NOW | RTLD_GLOBAL);
    void *vlc = core ? dlopen(argv[2], RTLD_NOW) : NULL;
    if (!vlc) {
        fprintf(stderr, "plugin-cache: %s\n", dlerror());
        return 1;
    }
    libvlc_new_fn create = (libvlc_new_fn)dlsym(vlc, "libvlc_new");
    libvlc_release_fn release = (libvlc_release_fn)dlsym(vlc, "libvlc_release");
    const char *const options[] = {"--quiet", "--reset-plugins-cache", "--no-ignore-config"};
    void *instance = create ? create(3, options) : NULL;
    if (!instance) {
        fprintf(stderr, "plugin-cache: libVLC didn't start\n");
        return 1;
    }
    release(instance);
    return 0;
}
