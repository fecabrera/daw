/* Independent ABI measurement through the C compiler, not bindgen assertions. */
#include "wrapper.h"
#include <stddef.h>
#define L(T) sizeof(T), _Alignof(T)
size_t daw_au_abi(size_t index) {
    AudioUnitParameterEvent event = {0};
    event.eventValues.ramp.startBufferOffset = -1;
    const size_t values[] = {
        L(AudioComponentDescription), L(AudioComponentPlugInInterface),
        L(AudioUnitParameterEvent), L(AudioBufferList), L(AudioBuffer),
        L(AudioTimeStamp), L(AudioStreamBasicDescription), L(HostCallbackInfo),
        L(AudioUnitCocoaViewInfo), L(AudioUnitParameterInfo), L(AURenderCallbackStruct),
        L(AudioUnitConnection), L(AudioUnitEvent), L(AUPreset),
        L(AudioUnitParameterStringFromValue), L(AudioUnitParameterValueFromString),
        offsetof(AudioBufferList,mBuffers), offsetof(AudioUnitParameterEvent,eventValues),
        offsetof(AudioUnitCocoaViewInfo,mCocoaAUViewClass),
        offsetof(AudioComponentPlugInInterface,Lookup),
        kAudioUnitRenderSelect, kAudioUnitProperty_StreamFormat,
        kAudioFormatFlagIsNonInterleaved, kAudioUnitParameterFlag_CanRamp,
        (size_t)kAudioUnitErr_CannotDoInCurrentContext,
        sizeof(((AudioUnitParameterEvent *)0)->eventValues.ramp.startBufferOffset),
        (size_t)(event.eventValues.ramp.startBufferOffset < 0)
    };
    return index < sizeof(values)/sizeof(values[0]) ? values[index] : (size_t)-1;
}
OSStatus daw_au_invoke_reset(AudioComponentPlugInInterface *instance, AudioUnitScope scope, AudioUnitElement element) {
    AudioUnitResetProc reset = (AudioUnitResetProc)instance->Lookup(kAudioUnitResetSelect);
    return reset ? reset(instance,scope,element) : kAudioUnitErr_InvalidProperty;
}
OSStatus daw_au_c_render(void *self, AudioUnitRenderActionFlags *flags, const AudioTimeStamp *time, UInt32 bus, UInt32 frames, AudioBufferList *buffers) {
    (void)self; (void)flags; (void)time; (void)bus;
    for (UInt32 channel=0;channel<buffers->mNumberBuffers;channel++) {
        if (buffers->mBuffers[channel].mDataByteSize < frames * sizeof(float)) return kAudio_ParamError;
        float *output=buffers->mBuffers[channel].mData;
        for (UInt32 frame=0;frame<frames;frame++) output[frame]=0.125f;
    }
    return noErr;
}
