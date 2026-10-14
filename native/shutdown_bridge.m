// GPUI 0.2.2 offers no veto for Dock/application Quit; delay it before its 100ms teardown.
#import <Cocoa/Cocoa.h>
#import <objc/runtime.h>
#include <stdatomic.h>

static atomic_bool requested = false;
static atomic_bool allowed = false;

static NSApplicationTerminateReply should_terminate(id self, SEL selector, NSApplication *app) {
    (void)self; (void)selector; (void)app;
    if (atomic_load(&allowed)) return NSTerminateNow;
    atomic_store(&requested, true);
    return NSTerminateCancel;
}

bool tasks_install_quit_guard(void) {
    id delegate = NSApp.delegate;
    if (!delegate) return false;
    // Replace as well as add: a delegate implementation must not silently bypass
    // coordinated shutdown. GPUI owns applicationWillTerminate:, left intact.
    class_replaceMethod(object_getClass(delegate), @selector(applicationShouldTerminate:),
                        (IMP)should_terminate, "Q@:@");
    return true;
}
bool tasks_quit_requested(void) { return atomic_exchange(&requested, false); }
void tasks_allow_quit(void) { atomic_store(&allowed, true); }
