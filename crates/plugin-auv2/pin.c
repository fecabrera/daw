// Hooks and Objective-C metadata must never point into an unloaded image.
#include <stdbool.h>
#include <dlfcn.h>
#include <mach-o/loader.h>
bool daw_au_pin_image(void) {
    Dl_info info;
    if (!dladdr((void *)&daw_au_pin_image,&info) || !info.dli_fname) return false;
    if (((const struct mach_header *)info.dli_fbase)->filetype==MH_EXECUTE) return true;
    return dlopen(info.dli_fname,RTLD_NOW|RTLD_NODELETE)!=NULL;
}
