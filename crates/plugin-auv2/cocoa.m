// Original Cocoa adapter. Product build scripts supply distinct static class
// names so NSBundle classNamed: can resolve each factory to its component image.
#import <AppKit/AppKit.h>
#import <AudioUnit/AUCocoaUIView.h>
#include <dlfcn.h>
#ifndef DAW_AU_FACTORY
#error Product-specific Cocoa class names are required
#endif

typedef void *(*Create)(AudioUnit,void *);
typedef void (*Retire)(void *);
typedef void (*Tick)(void *);
static struct { Create create; Retire retire; Tick tick; } callbacks;

@interface DAW_AU_CONTAINER : NSView {
@public
    void *_rustView;
    NSTimer *_timer;
}
- (void)sieTick:(NSTimer *)timer;
@end
@interface DAW_AU_TIMER : NSObject {
@public
    DAW_AU_CONTAINER *_target; // non-owning; timer is invalidated before target retirement
}
- (void)tick:(NSTimer *)timer;
@end
@interface DAW_AU_FACTORY : NSObject <AUCocoaUIBase>
@end

@implementation DAW_AU_CONTAINER
- (void)sieTick:(NSTimer *)timer {
    (void)timer;
    // A host listener can close the view during this callback. Defer dealloc,
    // including Rust context retirement, until the entire tick has returned.
    [self retain];
    if (_rustView && callbacks.tick) callbacks.tick(_rustView);
    [self release];
}
- (void)dealloc {
    [_timer invalidate]; [_timer release];
    if (_rustView && callbacks.retire) callbacks.retire(_rustView);
    [super dealloc];
}
@end
@implementation DAW_AU_TIMER
- (void)tick:(NSTimer *)timer { [_target sieTick:timer]; }
@end
@implementation DAW_AU_FACTORY
- (unsigned)interfaceVersion { return 0; }
- (NSString *)description { return [NSString stringWithString:NSStringFromClass([self class])]; }
- (NSView *)uiViewForAudioUnit:(AudioUnit)unit withSize:(NSSize)requested {
    (void)requested;
    if (![NSThread isMainThread] || !callbacks.create) return nil;
    DAW_AU_CONTAINER *view=[[DAW_AU_CONTAINER alloc] initWithFrame:NSMakeRect(0,0,100,100)];
    view->_rustView=callbacks.create(unit,view);
    if (!view->_rustView) { [view release]; return nil; }
    DAW_AU_TIMER *proxy=[[[DAW_AU_TIMER alloc] init] autorelease];proxy->_target=view;
    view->_timer=[[NSTimer timerWithTimeInterval:1.0/60.0 target:proxy selector:@selector(tick:) userInfo:nil repeats:YES] retain];
    [[NSRunLoop mainRunLoop] addTimer:view->_timer forMode:NSRunLoopCommonModes];
    return [view autorelease];
}
@end

void daw_au_register_view(const char *name,Create create,Retire retire,Tick tick) {
    // Called once, before publishing an instance. Classes are real image metadata;
    // dynamically allocated classes are not discoverable through NSBundle.
    @autoreleasepool {
        if (![NSStringFromClass([DAW_AU_FACTORY class]) isEqualToString:[NSString stringWithUTF8String:name]]) abort();
        callbacks.create=create;callbacks.retire=retire;callbacks.tick=tick;
    }
}
void daw_au_view_size(void *view,double width,double height) { [(NSView *)view setFrameSize:NSMakeSize(width,height)]; }
CFURLRef daw_au_bundle_url(void) { @autoreleasepool {
    Dl_info info;if(!dladdr((void *)&daw_au_bundle_url,&info)||!info.dli_fname)return NULL;
    NSString *path=[[NSString stringWithUTF8String:info.dli_fname] stringByDeletingLastPathComponent];
    path=[[path stringByDeletingLastPathComponent] stringByDeletingLastPathComponent];
    return (CFURLRef)[[NSURL fileURLWithPath:path isDirectory:YES] retain];
} }
