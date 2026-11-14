// Original headless host probe. Use real SDK calls on packaged binaries without
// installing components, opening an audio device or constructing a GPU editor.
#import <Foundation/Foundation.h>
#import <AudioToolbox/AudioToolbox.h>
#import <AudioToolbox/AUCocoaUIView.h>
#include <dlfcn.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#define CHECK(x) do { if (!(x)) { fprintf(stderr,"AU host probe failed at line %d: %s\n",__LINE__,#x); return 1; } } while(0)
static UInt32 code(NSString *text) {
    const unsigned char *s=(const unsigned char *)text.UTF8String;
    return (UInt32)s[0]<<24 | (UInt32)s[1]<<16 | (UInt32)s[2]<<8 | (UInt32)s[3];
}
static OSStatus input(void *data,AudioUnitRenderActionFlags *flags,const AudioTimeStamp *time,UInt32 bus,UInt32 frames,AudioBufferList *buffers) {
    (void)data;(void)flags;(void)time;(void)bus;
    for(UInt32 c=0;c<buffers->mNumberBuffers;c++) for(UInt32 n=0;n<frames;n++) ((float *)buffers->mBuffers[c].mData)[n]=0.1f*sinf((float)((time->mSampleTime+n)*6.283185307179586*220.0/48000.0));
    return noErr;
}
int main(int argc,const char **argv) { @autoreleasepool {
    CHECK(argc>1);
    for(int index=1;index<argc;index++) {
        NSString *path=[NSString stringWithUTF8String:argv[index]];
        NSDictionary *plist=[NSDictionary dictionaryWithContentsOfFile:[path stringByAppendingPathComponent:@"Contents/Info.plist"]];
        CHECK(plist!=nil);
        NSDictionary *au=[plist[@"AudioComponents"] firstObject];CHECK(au!=nil);
        NSString *binary=[[path stringByAppendingPathComponent:@"Contents/MacOS"] stringByAppendingPathComponent:plist[@"CFBundleExecutable"]];
        void *image=dlopen(binary.fileSystemRepresentation,RTLD_NOW|RTLD_LOCAL);
        if(!image) fprintf(stderr,"%s\n",dlerror());CHECK(image!=NULL);
        AudioComponentFactoryFunction factory=(AudioComponentFactoryFunction)dlsym(image,[au[@"factoryFunction"] UTF8String]);CHECK(factory!=NULL);
        AudioComponentDescription desc={code(au[@"type"]),code(au[@"subtype"]),code(au[@"manufacturer"]),0,0};
        AudioComponent component=AudioComponentRegister(&desc,(__bridge CFStringRef)au[@"name"],[au[@"version"] unsignedIntValue],factory);CHECK(component!=NULL);
        AudioUnit unit=NULL;CHECK(AudioComponentInstanceNew(component,&unit)==noErr);
        AudioUnitCocoaViewInfo view={0};UInt32 size=sizeof(view);
        CHECK(AudioUnitGetProperty(unit,kAudioUnitProperty_CocoaUI,kAudioUnitScope_Global,0,&view,&size)==noErr);
        CHECK([[(__bridge NSURL *)view.mCocoaAUViewBundleLocation path] isEqualToString:path]);
        NSBundle *viewBundle=[NSBundle bundleWithURL:(__bridge NSURL *)view.mCocoaAUViewBundleLocation];
        CHECK(viewBundle!=nil);
        Class cls=[viewBundle classNamed:(__bridge NSString *)view.mCocoaAUViewClass[0]];CHECK(cls!=Nil);
        id<AUCocoaUIBase> editor=[[[cls alloc] init] autorelease];CHECK([editor interfaceVersion]==0);
        CFRelease(view.mCocoaAUViewBundleLocation);CFRelease(view.mCocoaAUViewClass[0]);
        UInt32 bytes=0;Boolean writable=false;
        CHECK(AudioUnitGetPropertyInfo(unit,kAudioUnitProperty_ParameterList,0,0,&bytes,&writable)==noErr);
        CHECK(bytes>0 && bytes%sizeof(AudioUnitParameterID)==0 && bytes<=643*sizeof(AudioUnitParameterID));
        AudioUnitParameterID ids[643];size=bytes;CHECK(AudioUnitGetProperty(unit,kAudioUnitProperty_ParameterList,0,0,ids,&size)==noErr);
        for(UInt32 p=0;p<bytes/sizeof(AudioUnitParameterID);p++) {
            AudioUnitParameterInfo info={0};size=sizeof(info);CHECK(AudioUnitGetProperty(unit,kAudioUnitProperty_ParameterInfo,0,ids[p],&info,&size)==noErr);
            CHECK(info.minValue<=info.defaultValue && info.defaultValue<=info.maxValue);
            if(info.unit==kAudioUnitParameterUnit_Indexed) {
                float requested=info.minValue+(info.maxValue-info.minValue)*0.37f,actual=0;
                CHECK(AudioUnitSetParameter(unit,ids[p],0,0,requested,0)==noErr);
                CHECK(AudioUnitGetParameter(unit,ids[p],0,0,&actual)==noErr);CHECK(actual==roundf(requested));
            }
            CHECK(AudioUnitSetParameter(unit,ids[p],0,0,info.defaultValue,0)==noErr);
            if(info.flags & kAudioUnitParameterFlag_CFNameRelease) CFRelease(info.cfNameString);
        }
        UInt32 noAllocation=0;CHECK(AudioUnitSetProperty(unit,kAudioUnitProperty_ShouldAllocateBuffer,1,0,&noAllocation,sizeof(noAllocation))==noErr);
        CHECK(AudioUnitInitialize(unit)==noErr);
        AURenderCallbackStruct callback={input,NULL};CHECK(AudioUnitSetProperty(unit,kAudioUnitProperty_SetRenderCallback,kAudioUnitScope_Input,0,&callback,sizeof(callback))==noErr);
        float left[511]={0},right[511]={0};
        struct {UInt32 count;AudioBuffer planes[2];} storage={2,{{1,sizeof(left),left},{1,sizeof(right),right}}};
        AudioTimeStamp stamp={0};stamp.mFlags=kAudioTimeStampSampleTimeValid;AudioUnitRenderActionFlags flags=0;
        float peak=0;
        for(int block=0;block<96 && peak<=0.000001f;block++) {
            stamp.mSampleTime=block*511;flags=0;
            CHECK(AudioUnitRender(unit,&flags,&stamp,0,511,(AudioBufferList *)&storage)==noErr);
            for(int n=0;n<511;n++) {CHECK(isfinite(left[n]) && isfinite(right[n]));peak=fmaxf(peak,fmaxf(fabsf(left[n]),fabsf(right[n])));}
        }
        CHECK(peak>0.000001f);
        CFPropertyListRef state=NULL;size=sizeof(state);CHECK(AudioUnitGetProperty(unit,kAudioUnitProperty_ClassInfo,0,0,&state,&size)==noErr);
        CHECK(state && CFGetTypeID(state)==CFDictionaryGetTypeID());
        CHECK(AudioUnitSetProperty(unit,kAudioUnitProperty_ClassInfo,0,0,&state,sizeof(state))==noErr);CFRelease(state);
        CHECK(AudioUnitReset(unit,0,0)==noErr);CHECK(AudioUnitUninitialize(unit)==noErr);
        AudioUnit source=NULL;CHECK(AudioComponentInstanceNew(component,&source)==noErr);
        CHECK(AudioUnitSetProperty(source,kAudioUnitProperty_SetRenderCallback,kAudioUnitScope_Input,0,&callback,sizeof(callback))==noErr);
        CHECK(AudioUnitInitialize(source)==noErr);
        AudioUnitConnection connection={source,0,0};
        CHECK(AudioUnitInitialize(unit)==noErr);
        CHECK(AudioUnitSetProperty(unit,kAudioUnitProperty_MakeConnection,kAudioUnitScope_Input,0,&connection,sizeof(connection))==noErr);stamp.mSampleTime=511;flags=0;
        CHECK(AudioUnitRender(unit,&flags,&stamp,0,511,(AudioBufferList *)&storage)==noErr);
        for(int n=0;n<511;n++) CHECK(isfinite(left[n]) && isfinite(right[n]));
        CHECK(AudioUnitUninitialize(unit)==noErr);CHECK(AudioComponentInstanceDispose(unit)==noErr);
        CHECK(AudioUnitUninitialize(source)==noErr);CHECK(AudioComponentInstanceDispose(source)==noErr);
        CHECK(dlclose(image)==0);
        // The registered Objective-C factory must still have live image code.
        CHECK([editor interfaceVersion]==0);
        printf("PASS %s\n",argv[index]);
    }
    return 0;
} }
